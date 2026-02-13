import Foundation
import Virtualization

private enum NetMode: String {
  case none
  case nat
}

private struct ShareSpec {
  let tag: String
  let path: URL
  let readOnly: Bool
}

private struct LinuxBundle: Decodable {
  let kernel: String
  let rootfs: String
  let cmdline: String
}

private struct GuestBundleManifest: Decodable {
  let schema_version: String
  let linux: LinuxBundle
}

private struct RunArgs {
  var runId: String
  var bundleDir: URL
  var stateDir: URL
  var shares: [ShareSpec]
  var cpus: Int
  var memBytes: UInt64
  var net: NetMode
  var wallMs: UInt64
  var graceMs: UInt64
}

private func usage(_ out: FileHandle) {
  let msg = """
  usage:
    x07-vz-helper run --run-id ID --bundle DIR --state-dir DIR --share TAG PATH ro|rw... [--cpus N] [--mem-bytes BYTES] [--net none|nat] [--wall-ms MS] [--grace-ms MS]
    x07-vz-helper preflight

  notes:
    - Requires codesign entitlement: com.apple.security.virtualization=true
    - Shares are VirtioFS tags (x07in, x07out, x07m0..x07m63).
  """
  out.write(msg.data(using: .utf8)!)
}

private func die(_ code: Int32, _ msg: String) -> Never {
  FileHandle.standardError.write((msg + "\n").data(using: .utf8)!)
  exit(code)
}

private func parseRunArgs(_ argv: [String]) -> RunArgs {
  var runId: String?
  var bundleDir: URL?
  var stateDir: URL?
  var shares: [ShareSpec] = []
  var cpus = 2
  var memBytes: UInt64 = 512 * 1024 * 1024
  var net: NetMode = .none
  var wallMs: UInt64 = 30_000
  var graceMs: UInt64 = 1_000

  var i = 0
  while i < argv.count {
    let a = argv[i]
    switch a {
    case "--run-id":
      i += 1
      guard i < argv.count else { die(2, "--run-id requires a value") }
      runId = argv[i]

    case "--bundle":
      i += 1
      guard i < argv.count else { die(2, "--bundle requires a value") }
      bundleDir = URL(fileURLWithPath: argv[i])

    case "--state-dir":
      i += 1
      guard i < argv.count else { die(2, "--state-dir requires a value") }
      stateDir = URL(fileURLWithPath: argv[i])

    case "--share":
      guard i + 3 < argv.count else { die(2, "--share requires: TAG PATH ro|rw") }
      let tag = argv[i + 1]
      let path = URL(fileURLWithPath: argv[i + 2])
      let mode = argv[i + 3]
      let readOnly: Bool
      if mode == "ro" {
        readOnly = true
      } else if mode == "rw" {
        readOnly = false
      } else {
        die(2, "invalid share mode \(mode) (expected ro|rw)")
      }
      shares.append(ShareSpec(tag: tag, path: path, readOnly: readOnly))
      i += 3

    case "--cpus":
      i += 1
      guard i < argv.count, let n = Int(argv[i]), n > 0 else { die(2, "--cpus requires a positive int") }
      cpus = n

    case "--mem-bytes":
      i += 1
      guard i < argv.count, let n = UInt64(argv[i]) else { die(2, "--mem-bytes requires a uint64") }
      memBytes = n

    case "--net":
      i += 1
      guard i < argv.count, let m = NetMode(rawValue: argv[i]) else { die(2, "--net requires one of: none, nat") }
      net = m

    case "--wall-ms":
      i += 1
      guard i < argv.count, let n = UInt64(argv[i]), n > 0 else { die(2, "--wall-ms requires a positive int") }
      wallMs = n

    case "--grace-ms":
      i += 1
      guard i < argv.count, let n = UInt64(argv[i]) else { die(2, "--grace-ms requires a uint64") }
      graceMs = n

    case "--help", "-h":
      usage(FileHandle.standardOutput)
      exit(0)

    default:
      die(2, "unknown arg: \(a)")
    }
    i += 1
  }

  guard let b = bundleDir else { die(2, "missing --bundle") }
  guard let s = stateDir else { die(2, "missing --state-dir") }
  guard let rid = runId, !rid.isEmpty else { die(2, "missing --run-id") }
  if shares.isEmpty { die(2, "at least one --share is required") }

  return RunArgs(runId: rid, bundleDir: b, stateDir: s, shares: shares, cpus: cpus, memBytes: memBytes, net: net, wallMs: wallMs, graceMs: graceMs)
}

private func readManifest(_ bundleDir: URL) throws -> GuestBundleManifest {
  let data = try Data(contentsOf: bundleDir.appendingPathComponent("manifest.json"))
  let decoder = JSONDecoder()
  return try decoder.decode(GuestBundleManifest.self, from: data)
}

private func roundUpMiB(_ bytes: UInt64) -> UInt64 {
  let mib: UInt64 = 1024 * 1024
  if bytes == 0 { return mib }
  let r = (bytes + (mib - 1)) / mib
  return r * mib
}

private func writeAll(_ fd: Int32, _ buf: UnsafeRawPointer, _ count: Int) {
  var off = 0
  while off < count {
    let n = Darwin.write(fd, buf.advanced(by: off), count - off)
    if n <= 0 { break }
    off += n
  }
}

private final class ConnectionState {
  let lock = NSLock()
  var stdoutConn: VZVirtioSocketConnection?
  var stderrConn: VZVirtioSocketConnection?
  var ctrlConn: VZVirtioSocketConnection?

  var ctrlExitCode: Int32?
  var ctrlFlags: UInt32?

  let ctrlDone = DispatchSemaphore(value: 0)
  let stdoutDone = DispatchSemaphore(value: 0)
  let stderrDone = DispatchSemaphore(value: 0)
}

private final class VsockDelegate: NSObject, VZVirtioSocketListenerDelegate {
  private let state: ConnectionState

  init(state: ConnectionState) {
    self.state = state
  }

  func listener(_ listener: VZVirtioSocketListener, shouldAcceptNewConnection connection: VZVirtioSocketConnection, from socketDevice: VZVirtioSocketDevice) -> Bool {
    let port = connection.destinationPort

    state.lock.lock()
    defer { state.lock.unlock() }

    if port == 5000 {
      if state.stdoutConn != nil { return false }
      state.stdoutConn = connection
      DispatchQueue.global().async { self.pumpStream(connection, outFd: STDOUT_FILENO, done: self.state.stdoutDone) }
      return true
    }
    if port == 5001 {
      if state.stderrConn != nil { return false }
      state.stderrConn = connection
      DispatchQueue.global().async { self.pumpStream(connection, outFd: STDERR_FILENO, done: self.state.stderrDone) }
      return true
    }
    if port == 5002 {
      if state.ctrlConn != nil { return false }
      state.ctrlConn = connection
      DispatchQueue.global().async { self.readCtrl(connection, done: self.state.ctrlDone) }
      return true
    }

    return false
  }

  private func pumpStream(_ conn: VZVirtioSocketConnection, outFd: Int32, done: DispatchSemaphore) {
    defer { done.signal() }

    let inFd = conn.fileDescriptor
    if inFd < 0 { return }

    var buf = [UInt8](repeating: 0, count: 16 * 1024)
    while true {
      let n = buf.withUnsafeMutableBytes { ptr in
        Darwin.read(inFd, ptr.baseAddress, ptr.count)
      }
      if n <= 0 { break }
      buf.withUnsafeBytes { ptr in
        writeAll(outFd, ptr.baseAddress!, n)
      }
    }
  }

  private func readExact(_ fd: Int32, _ count: Int) -> Data? {
    var out = Data(count: count)
    let ok: Bool = out.withUnsafeMutableBytes { ptr in
      guard let base = ptr.baseAddress else { return false }
      var off = 0
      while off < count {
        let n = Darwin.read(fd, base.advanced(by: off), count - off)
        if n <= 0 { return false }
        off += n
      }
      return true
    }
    return ok ? out : nil
  }

  private func readCtrl(_ conn: VZVirtioSocketConnection, done: DispatchSemaphore) {
    defer { done.signal() }
    let fd = conn.fileDescriptor
    if fd < 0 { return }

    guard let base = readExact(fd, 8) else { return }
    let exitCode = base.withUnsafeBytes { ptr -> Int32 in
      ptr.load(fromByteOffset: 0, as: Int32.self).littleEndian
    }
    let flags = base.withUnsafeBytes { ptr -> UInt32 in
      ptr.load(fromByteOffset: 4, as: UInt32.self).littleEndian
    }

    state.lock.lock()
    state.ctrlExitCode = exitCode
    state.ctrlFlags = flags
    state.lock.unlock()

    if (flags & 0x00000010) != 0 {
      _ = readExact(fd, 24) // best-effort
    }
  }
}

private func cloneRootfsImage(base: URL, toDir: URL) throws -> URL {
  try FileManager.default.createDirectory(at: toDir, withIntermediateDirectories: true)
  let dst = toDir.appendingPathComponent("rootfs.cow.img")

  if FileManager.default.fileExists(atPath: dst.path) {
    try FileManager.default.removeItem(at: dst)
  }

  let flags = copyfile_flags_t(COPYFILE_CLONE | COPYFILE_UNLINK)
  if copyfile(base.path, dst.path, nil, flags) != 0 {
    let fallback = copyfile_flags_t(COPYFILE_ALL | COPYFILE_UNLINK)
    if copyfile(base.path, dst.path, nil, fallback) != 0 {
      throw NSError(domain: NSPOSIXErrorDomain, code: Int(errno))
    }
  }
  return dst
}

private func buildConfig(_ args: RunArgs, manifest: GuestBundleManifest) throws -> (VZVirtualMachineConfiguration, URL) {
  let kernelURL = args.bundleDir.appendingPathComponent(manifest.linux.kernel)
  let cmdlineURL = args.bundleDir.appendingPathComponent(manifest.linux.cmdline)
  let baseRootfsURL = args.bundleDir.appendingPathComponent(manifest.linux.rootfs)

  let baseCmdline = try String(contentsOf: cmdlineURL, encoding: .utf8).trimmingCharacters(in: .whitespacesAndNewlines)
  let cmdline = baseCmdline + " x07.run_id=" + args.runId

  let cowRootfs = try cloneRootfsImage(base: baseRootfsURL, toDir: args.stateDir)

  let bootLoader = VZLinuxBootLoader(kernelURL: kernelURL)
  bootLoader.commandLine = cmdline

  let config = VZVirtualMachineConfiguration()
  config.bootLoader = bootLoader

  let mem = roundUpMiB(args.memBytes)
  config.memorySize = max(VZVirtualMachineConfiguration.minimumAllowedMemorySize, min(mem, VZVirtualMachineConfiguration.maximumAllowedMemorySize))
  let cpu = max(VZVirtualMachineConfiguration.minimumAllowedCPUCount, min(args.cpus, VZVirtualMachineConfiguration.maximumAllowedCPUCount))
  config.cpuCount = cpu

  do {
    let attachment = try VZDiskImageStorageDeviceAttachment(
      url: cowRootfs,
      readOnly: false,
      cachingMode: .cached,
      synchronizationMode: .none
    )
    config.storageDevices = [VZVirtioBlockDeviceConfiguration(attachment: attachment)]
  }

  var dirDevices: [VZDirectorySharingDeviceConfiguration] = []
  for s in args.shares {
    try VZVirtioFileSystemDeviceConfiguration.validateTag(s.tag)

    var isDir: ObjCBool = false
    guard FileManager.default.fileExists(atPath: s.path.path, isDirectory: &isDir), isDir.boolValue else {
      throw NSError(domain: NSPOSIXErrorDomain, code: Int(ENOENT))
    }

    let shared = VZSharedDirectory(url: s.path, readOnly: s.readOnly)
    let share = VZSingleDirectoryShare(directory: shared)

    let fs = VZVirtioFileSystemDeviceConfiguration(tag: s.tag)
    fs.share = share
    dirDevices.append(fs)
  }
  config.directorySharingDevices = dirDevices

  config.entropyDevices = [VZVirtioEntropyDeviceConfiguration()]
  config.memoryBalloonDevices = [VZVirtioTraditionalMemoryBalloonDeviceConfiguration()]

  let socketConfig = VZVirtioSocketDeviceConfiguration()
  config.socketDevices = [socketConfig]

  if args.net == .nat {
    let net = VZVirtioNetworkDeviceConfiguration()
    net.attachment = VZNATNetworkDeviceAttachment()
    config.networkDevices = [net]
  } else {
    config.networkDevices = []
  }

  let serial = VZVirtioConsoleDeviceSerialPortConfiguration()
  serial.attachment = VZFileHandleSerialPortAttachment(fileHandleForReading: nil, fileHandleForWriting: FileHandle.standardError)
  config.serialPorts = [serial]

  return (config, cowRootfs)
}

private func run(_ args: RunArgs) throws -> Int32 {
  let manifest = try readManifest(args.bundleDir)
  guard manifest.schema_version == "x07.vz.guest.bundle@0.1.0" else {
    throw NSError(domain: "x07-vz-helper", code: 10)
  }

  let (config, cowRootfs) = try buildConfig(args, manifest: manifest)
  defer { try? FileManager.default.removeItem(at: cowRootfs) }

  try config.validate()

  let queue = DispatchQueue(label: "x07.vz.vm")
  let vm = VZVirtualMachine(configuration: config, queue: queue)

  let state = ConnectionState()
  let listener = VZVirtioSocketListener()
  let delegate = VsockDelegate(state: state)
  listener.delegate = delegate

  queue.sync {
    if let dev = vm.socketDevices.first as? VZVirtioSocketDevice {
      dev.setSocketListener(listener, forPort: 5000)
      dev.setSocketListener(listener, forPort: 5001)
      dev.setSocketListener(listener, forPort: 5002)
    }
  }

  let startSem = DispatchSemaphore(value: 0)
  var startErr: Error?
  queue.async {
    vm.start { result in
      if case .failure(let e) = result {
        startErr = e
      }
      startSem.signal()
    }
  }
  startSem.wait()
  if let e = startErr { throw e }

  let softAt = args.wallMs.saturatingSubtracting(args.graceMs)
  let softTimer = DispatchSource.makeTimerSource()
  softTimer.schedule(deadline: .now() + .milliseconds(Int(softAt)))
  softTimer.setEventHandler {
    queue.async {
      _ = try? vm.requestStop()
    }
  }
  softTimer.resume()

  let hardTimer = DispatchSource.makeTimerSource()
  hardTimer.schedule(deadline: .now() + .milliseconds(Int(args.wallMs)))
  hardTimer.setEventHandler {
    queue.async {
      vm.stop { _ in }
    }
  }
  hardTimer.resume()

  _ = state.ctrlDone.wait(timeout: .now() + .milliseconds(Int(args.wallMs)))

  // Give stdout/stderr a moment to flush after CTRL.
  _ = state.stdoutDone.wait(timeout: .now() + .milliseconds(200))
  _ = state.stderrDone.wait(timeout: .now() + .milliseconds(200))

  softTimer.cancel()
  hardTimer.cancel()

  let stopSem = DispatchSemaphore(value: 0)
  queue.async {
    vm.stop { _ in
      stopSem.signal()
    }
  }
  _ = stopSem.wait(timeout: .now() + .seconds(2))

  state.lock.lock()
  let exitCode = state.ctrlExitCode ?? 124
  state.lock.unlock()
  return exitCode
}

private func preflight() -> Int32 {
  if !VZVirtualMachine.isSupported {
    FileHandle.standardError.write("vz: not supported\n".data(using: .utf8)!)
    return 2
  }
  FileHandle.standardOutput.write("ok: vz supported\n".data(using: .utf8)!)
  return 0
}

let argv = Array(CommandLine.arguments.dropFirst())
if argv.isEmpty {
  usage(FileHandle.standardError)
  exit(2)
}

let sub = argv[0]
if sub == "preflight" {
  exit(preflight())
}
if sub != "run" {
  usage(FileHandle.standardError)
  exit(2)
}

do {
  let runArgs = parseRunArgs(Array(argv.dropFirst()))
  let code = try run(runArgs)
  exit(code)
} catch {
  die(2, "vz helper failed: \(error)")
}

private extension UInt64 {
  func saturatingSubtracting(_ other: UInt64) -> UInt64 {
    return self > other ? (self - other) : 0
  }
}
