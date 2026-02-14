use std::path::Path;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};

use crate::{
    firecracker_ctr_config_from_env, firecracker_ctr_config_from_job, hard_kill_pid_and_group,
    run_command_capped, vz_cleanup_scratch, FirecrackerCtrConfig, VmBackend, VmJob,
};

#[derive(Debug, Clone, Copy)]
struct KillSchedule {
    t_soft: Instant,
    t_hard: Instant,
    t_cleanup_deadline: Instant,
}

impl KillSchedule {
    fn from_plan(plan: &KillPlan) -> Self {
        let now_unix_ms = now_unix_ms().unwrap_or(u64::MAX);
        let now = Instant::now();

        let soft_in = Duration::from_millis(plan.t_soft_unix_ms.saturating_sub(now_unix_ms));
        let hard_in = Duration::from_millis(plan.t_hard_unix_ms.saturating_sub(now_unix_ms));
        let cleanup_in =
            Duration::from_millis(plan.t_cleanup_deadline_unix_ms.saturating_sub(now_unix_ms));

        let t_soft = now.checked_add(soft_in).unwrap_or(now);
        let t_hard = now.checked_add(hard_in).unwrap_or(now);
        let t_cleanup_deadline = now.checked_add(cleanup_in).unwrap_or(now);

        KillSchedule {
            t_soft,
            t_hard,
            t_cleanup_deadline,
        }
    }
}

#[derive(Debug, Clone)]
pub struct CommandSpec {
    pub program: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub timeout: Duration,
    pub best_effort: bool,
}

#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_status: i32,
    pub timed_out: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl ExecResult {
    pub fn ok(&self) -> bool {
        !self.timed_out && self.exit_status == 0
    }

    pub fn not_found_or_gone(&self) -> bool {
        if self.ok() {
            return false;
        }
        if self.timed_out {
            return false;
        }
        let s = String::from_utf8_lossy(&self.stderr).to_ascii_lowercase();
        s.contains("not found")
            || s.contains("no such")
            || s.contains("does not exist")
            || s.contains("not exist")
            || s.contains("unknown container")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Term,
    Kill,
}

impl Signal {
    fn for_mac_container(self) -> &'static str {
        match self {
            Signal::Term => "SIGTERM",
            Signal::Kill => "KILL",
        }
    }

    fn for_docker_like(self) -> &'static str {
        match self {
            Signal::Term => "SIGTERM",
            Signal::Kill => "SIGKILL",
        }
    }

    fn for_ctr_like(self) -> &'static str {
        match self {
            Signal::Term => "SIGTERM",
            Signal::Kill => "SIGKILL",
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct RetryPolicy {
    pub initial: Duration,
    pub max: Duration,
}

impl RetryPolicy {
    pub fn default_for_reaper() -> Self {
        RetryPolicy {
            initial: Duration::from_millis(100),
            max: Duration::from_secs(1),
        }
    }
}

#[derive(Debug, Clone)]
pub struct TargetRef {
    pub id: String,
}

#[derive(Debug, Clone)]
pub struct KillPlan {
    pub backend: VmBackend,
    pub target: TargetRef,

    pub t_soft_unix_ms: u64,
    pub t_hard_unix_ms: u64,
    pub t_cleanup_deadline_unix_ms: u64,

    pub soft_signal: Signal,
    pub hard_signal: Signal,
    pub grace: Duration,
    pub cleanup_budget: Duration,
    pub op_timeout: Duration,
    pub retry: RetryPolicy,
}

impl KillPlan {
    pub fn from_job(job: &VmJob) -> Self {
        let t_soft = job
            .deadline_unix_ms
            .saturating_sub(job.grace_ms)
            .max(job.created_unix_ms);
        let t_hard = job.deadline_unix_ms;
        let t_cleanup_deadline = job.deadline_unix_ms.saturating_add(job.cleanup_ms);

        KillPlan {
            backend: job.backend,
            target: TargetRef {
                id: job.container_id.clone(),
            },
            t_soft_unix_ms: t_soft,
            t_hard_unix_ms: t_hard,
            t_cleanup_deadline_unix_ms: t_cleanup_deadline,
            soft_signal: Signal::Term,
            hard_signal: Signal::Kill,
            grace: Duration::from_millis(job.grace_ms.max(1)),
            cleanup_budget: Duration::from_millis(job.cleanup_ms),
            op_timeout: Duration::from_secs(2),
            retry: RetryPolicy::default_for_reaper(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KillResult {
    CompletedBeforeDeadline,
    KilledAtHardDeadline,
    CleanupTimeout,
}

pub trait KillBackend {
    fn build_soft_stop(
        &self,
        t: &TargetRef,
        sig: Signal,
        grace: Duration,
        op_timeout: Duration,
    ) -> Vec<CommandSpec>;

    fn build_hard_kill(&self, t: &TargetRef, sig: Signal, op_timeout: Duration)
        -> Vec<CommandSpec>;

    fn build_cleanup(&self, t: &TargetRef, op_timeout: Duration) -> Vec<CommandSpec>;

    fn build_probe(&self, t: &TargetRef, op_timeout: Duration) -> Option<CommandSpec>;
}

#[derive(Debug, Clone)]
struct MacContainerCli {
    bin: String,
}

impl MacContainerCli {
    fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }
}

impl KillBackend for MacContainerCli {
    fn build_soft_stop(
        &self,
        t: &TargetRef,
        sig: Signal,
        _grace: Duration,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec![
                "kill".to_string(),
                "--signal".to_string(),
                sig.for_mac_container().to_string(),
                t.id.clone(),
            ],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        }]
    }

    fn build_hard_kill(
        &self,
        t: &TargetRef,
        sig: Signal,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec![
                "kill".to_string(),
                "--signal".to_string(),
                sig.for_mac_container().to_string(),
                t.id.clone(),
            ],
            env: vec![],
            timeout: op_timeout,
            best_effort: false,
        }]
    }

    fn build_cleanup(&self, t: &TargetRef, op_timeout: Duration) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec!["delete".to_string(), "--force".to_string(), t.id.clone()],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        }]
    }

    fn build_probe(&self, t: &TargetRef, op_timeout: Duration) -> Option<CommandSpec> {
        Some(CommandSpec {
            program: self.bin.clone(),
            args: vec!["inspect".to_string(), t.id.clone()],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        })
    }
}

#[derive(Debug, Clone)]
struct DockerLikeCli {
    bin: String,
}

impl DockerLikeCli {
    fn new(bin: impl Into<String>) -> Self {
        Self { bin: bin.into() }
    }
}

impl KillBackend for DockerLikeCli {
    fn build_soft_stop(
        &self,
        t: &TargetRef,
        _sig: Signal,
        grace: Duration,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        let secs = (grace.as_millis().saturating_add(999) / 1000).max(1);
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec![
                "stop".to_string(),
                "--time".to_string(),
                secs.to_string(),
                t.id.clone(),
            ],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        }]
    }

    fn build_hard_kill(
        &self,
        t: &TargetRef,
        sig: Signal,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec![
                "kill".to_string(),
                "--signal".to_string(),
                sig.for_docker_like().to_string(),
                t.id.clone(),
            ],
            env: vec![],
            timeout: op_timeout,
            best_effort: false,
        }]
    }

    fn build_cleanup(&self, t: &TargetRef, op_timeout: Duration) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: vec!["rm".to_string(), "-f".to_string(), t.id.clone()],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        }]
    }

    fn build_probe(&self, t: &TargetRef, op_timeout: Duration) -> Option<CommandSpec> {
        Some(CommandSpec {
            program: self.bin.clone(),
            args: vec!["inspect".to_string(), t.id.clone()],
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        })
    }
}

#[derive(Debug, Clone)]
struct CtrLike {
    bin: String,
    address: String,
    namespace: String,
}

impl CtrLike {
    fn from_firecracker_cfg(cfg: &FirecrackerCtrConfig) -> Self {
        Self {
            bin: cfg.bin.to_string_lossy().to_string(),
            address: cfg.address.clone(),
            namespace: cfg.namespace.clone(),
        }
    }

    fn base_args(&self, op_timeout: Duration) -> Vec<String> {
        vec![
            "--address".to_string(),
            self.address.clone(),
            "--namespace".to_string(),
            self.namespace.clone(),
            "--timeout".to_string(),
            crate::duration_to_ctr_timeout_arg(op_timeout)
                .to_string_lossy()
                .to_string(),
        ]
    }
}

impl KillBackend for CtrLike {
    fn build_soft_stop(
        &self,
        t: &TargetRef,
        sig: Signal,
        _grace: Duration,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: self
                .base_args(op_timeout)
                .into_iter()
                .chain(vec![
                    "tasks".to_string(),
                    "kill".to_string(),
                    "--all".to_string(),
                    "--signal".to_string(),
                    sig.for_ctr_like().to_string(),
                    t.id.clone(),
                ])
                .collect(),
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        }]
    }

    fn build_hard_kill(
        &self,
        t: &TargetRef,
        sig: Signal,
        op_timeout: Duration,
    ) -> Vec<CommandSpec> {
        vec![CommandSpec {
            program: self.bin.clone(),
            args: self
                .base_args(op_timeout)
                .into_iter()
                .chain(vec![
                    "tasks".to_string(),
                    "kill".to_string(),
                    "--all".to_string(),
                    "--signal".to_string(),
                    sig.for_ctr_like().to_string(),
                    t.id.clone(),
                ])
                .collect(),
            env: vec![],
            timeout: op_timeout,
            best_effort: false,
        }]
    }

    fn build_cleanup(&self, t: &TargetRef, op_timeout: Duration) -> Vec<CommandSpec> {
        vec![
            CommandSpec {
                program: self.bin.clone(),
                args: self
                    .base_args(op_timeout)
                    .into_iter()
                    .chain(vec![
                        "tasks".to_string(),
                        "delete".to_string(),
                        "--force".to_string(),
                        t.id.clone(),
                    ])
                    .collect(),
                env: vec![],
                timeout: op_timeout,
                best_effort: true,
            },
            CommandSpec {
                program: self.bin.clone(),
                args: self
                    .base_args(op_timeout)
                    .into_iter()
                    .chain(vec![
                        "containers".to_string(),
                        "delete".to_string(),
                        t.id.clone(),
                    ])
                    .collect(),
                env: vec![],
                timeout: op_timeout,
                best_effort: true,
            },
        ]
    }

    fn build_probe(&self, t: &TargetRef, op_timeout: Duration) -> Option<CommandSpec> {
        Some(CommandSpec {
            program: self.bin.clone(),
            args: self
                .base_args(op_timeout)
                .into_iter()
                .chain(vec![
                    "containers".to_string(),
                    "info".to_string(),
                    t.id.clone(),
                ])
                .collect(),
            env: vec![],
            timeout: op_timeout,
            best_effort: true,
        })
    }
}

pub fn enforce_kill_plan<FRun, FDone>(
    plan: &KillPlan,
    backend: &dyn KillBackend,
    mut run_cmd: FRun,
    mut is_done: FDone,
) -> KillResult
where
    FRun: FnMut(CommandSpec) -> ExecResult,
    FDone: FnMut() -> bool,
{
    let schedule = KillSchedule::from_plan(plan);

    let mut soft_done = false;
    let mut hard_done = false;
    let mut cleanup_backoff = plan.retry.initial;

    loop {
        if is_done() {
            return KillResult::CompletedBeforeDeadline;
        }

        let now = Instant::now();

        if !soft_done && now < schedule.t_soft {
            std::thread::sleep(
                schedule
                    .t_soft
                    .saturating_duration_since(now)
                    .min(Duration::from_millis(250))
                    .max(Duration::from_millis(1)),
            );
            continue;
        }

        if now >= schedule.t_soft || soft_done {
            if let Some(probe) = backend.build_probe(&plan.target, plan.op_timeout) {
                let pr = run_cmd(probe);
                if pr.not_found_or_gone() {
                    return if hard_done {
                        KillResult::KilledAtHardDeadline
                    } else {
                        KillResult::CompletedBeforeDeadline
                    };
                }
            }
        }

        if !soft_done && now >= schedule.t_soft && now < schedule.t_hard {
            run_seq(
                schedule,
                backend.build_soft_stop(
                    &plan.target,
                    plan.soft_signal,
                    plan.grace,
                    plan.op_timeout,
                ),
                &mut run_cmd,
            );
            soft_done = true;
        }

        if !hard_done && now >= schedule.t_hard {
            run_seq(
                schedule,
                backend.build_hard_kill(&plan.target, plan.hard_signal, plan.op_timeout),
                &mut run_cmd,
            );
            hard_done = true;
            cleanup_backoff = plan.retry.initial;
        }

        if hard_done {
            run_seq(
                schedule,
                backend.build_cleanup(&plan.target, plan.op_timeout),
                &mut run_cmd,
            );
            if Instant::now() >= schedule.t_cleanup_deadline {
                return KillResult::CleanupTimeout;
            }
            std::thread::sleep(cleanup_backoff);
            cleanup_backoff = (cleanup_backoff * 2).min(plan.retry.max);
            continue;
        }

        let next = if !soft_done {
            schedule.t_soft
        } else {
            schedule.t_hard
        };
        std::thread::sleep(
            next.saturating_duration_since(now)
                .min(Duration::from_millis(250))
                .max(Duration::from_millis(1)),
        );
    }
}

fn run_seq<FRun>(schedule: KillSchedule, seq: Vec<CommandSpec>, run_cmd: &mut FRun)
where
    FRun: FnMut(CommandSpec) -> ExecResult,
{
    for mut c in seq {
        let remaining = schedule
            .t_cleanup_deadline
            .saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return;
        }
        c.timeout = c.timeout.min(remaining);
        if c.timeout.is_zero() {
            return;
        }
        let _ = run_cmd(c);
    }
}

pub fn enforce_kill_plan_for_job(
    job: &VmJob,
    state_dir: &Path,
    done_marker: &Path,
) -> Result<KillResult> {
    let plan = KillPlan::from_job(job);
    let is_done = || done_marker.is_file();

    match job.backend {
        VmBackend::Vz => enforce_vz_kill(job, state_dir, done_marker),
        VmBackend::AppleContainer => Ok(enforce_kill_plan(
            &plan,
            &MacContainerCli::new("container"),
            run_command_spec,
            is_done,
        )),
        VmBackend::Docker => Ok(enforce_kill_plan(
            &plan,
            &DockerLikeCli::new("docker"),
            run_command_spec,
            is_done,
        )),
        VmBackend::Podman => Ok(enforce_kill_plan(
            &plan,
            &DockerLikeCli::new("podman"),
            run_command_spec,
            is_done,
        )),
        VmBackend::FirecrackerCtr => {
            let cfg = job
                .ctr
                .as_ref()
                .map(firecracker_ctr_config_from_job)
                .unwrap_or_else(firecracker_ctr_config_from_env);
            let backend = CtrLike::from_firecracker_cfg(&cfg);
            Ok(enforce_kill_plan(
                &plan,
                &backend,
                run_command_spec,
                is_done,
            ))
        }
    }
}

fn enforce_vz_kill(job: &VmJob, state_dir: &Path, done_marker: &Path) -> Result<KillResult> {
    let plan = KillPlan::from_job(job);
    let schedule = KillSchedule::from_plan(&plan);

    if !done_marker.is_file() && Instant::now() < schedule.t_soft {
        sleep_until_or_done(schedule.t_soft, done_marker)?;
    }
    if done_marker.is_file() {
        return Ok(KillResult::CompletedBeforeDeadline);
    }

    sleep_until_or_done(schedule.t_hard, done_marker)?;
    if done_marker.is_file() {
        return Ok(KillResult::CompletedBeforeDeadline);
    }

    let Some(pid) = job.pid else {
        let _ = vz_cleanup_scratch(state_dir);
        return Ok(KillResult::CleanupTimeout);
    };

    let mut backoff = plan.retry.initial;
    loop {
        if done_marker.is_file() {
            return Ok(KillResult::CompletedBeforeDeadline);
        }

        if is_pid_gone(pid) {
            let _ = vz_cleanup_scratch(state_dir);
            return Ok(KillResult::KilledAtHardDeadline);
        }

        hard_kill_pid_and_group(pid);
        let _ = vz_cleanup_scratch(state_dir);

        if Instant::now() >= schedule.t_cleanup_deadline {
            return Ok(KillResult::CleanupTimeout);
        }

        std::thread::sleep(backoff);
        backoff = (backoff * 2).min(plan.retry.max);
    }
}

fn now_unix_ms() -> Result<u64> {
    let d = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .context("system time before unix epoch")?;
    Ok(d.as_millis().try_into().unwrap_or(u64::MAX))
}

fn is_pid_gone(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let Ok(pid_i32) = i32::try_from(pid) else {
            return false;
        };
        unsafe {
            let pid_gone = match libc::kill(pid_i32, 0) {
                0 => false,
                _ => std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
            };
            let pg_gone = match libc::kill(-pid_i32, 0) {
                0 => false,
                _ => std::io::Error::last_os_error().raw_os_error() == Some(libc::ESRCH),
            };
            pid_gone && pg_gone
        }
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

fn sleep_until_or_done(deadline: Instant, done_marker: &Path) -> Result<()> {
    loop {
        if done_marker.is_file() {
            return Ok(());
        }
        let now = Instant::now();
        if now >= deadline {
            return Ok(());
        }
        let remaining = deadline.saturating_duration_since(now);
        std::thread::sleep(remaining.min(Duration::from_millis(250)));
    }
}

fn run_command_spec(spec: CommandSpec) -> ExecResult {
    let mut cmd = std::process::Command::new(&spec.program);
    cmd.args(&spec.args);
    cmd.stdin(std::process::Stdio::null());
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());
    cmd.env("LC_ALL", "C");
    for (k, v) in spec.env {
        cmd.env(k, v);
    }

    let wall_ms: u64 = spec.timeout.as_millis().try_into().unwrap_or(u64::MAX);
    let out = run_command_capped(cmd, wall_ms.max(1), 64 * 1024, 64 * 1024).unwrap_or_else(|_| {
        crate::RunOutput {
            exit_status: 1,
            timed_out: true,
            stdout: Vec::new(),
            stderr: Vec::new(),
            stdout_truncated: false,
            stderr_truncated: false,
        }
    });

    ExecResult {
        exit_status: out.exit_status,
        timed_out: out.timed_out,
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    struct FakeBackend;

    impl KillBackend for FakeBackend {
        fn build_soft_stop(
            &self,
            t: &TargetRef,
            _sig: Signal,
            _grace: Duration,
            op_timeout: Duration,
        ) -> Vec<CommandSpec> {
            vec![CommandSpec {
                program: "soft".to_string(),
                args: vec![t.id.clone()],
                env: vec![],
                timeout: op_timeout,
                best_effort: true,
            }]
        }

        fn build_hard_kill(
            &self,
            t: &TargetRef,
            _sig: Signal,
            op_timeout: Duration,
        ) -> Vec<CommandSpec> {
            vec![CommandSpec {
                program: "hard".to_string(),
                args: vec![t.id.clone()],
                env: vec![],
                timeout: op_timeout,
                best_effort: false,
            }]
        }

        fn build_cleanup(&self, t: &TargetRef, op_timeout: Duration) -> Vec<CommandSpec> {
            vec![CommandSpec {
                program: "cleanup".to_string(),
                args: vec![t.id.clone()],
                env: vec![],
                timeout: op_timeout,
                best_effort: true,
            }]
        }

        fn build_probe(&self, t: &TargetRef, op_timeout: Duration) -> Option<CommandSpec> {
            Some(CommandSpec {
                program: "probe".to_string(),
                args: vec![t.id.clone()],
                env: vec![],
                timeout: op_timeout,
                best_effort: true,
            })
        }
    }

    #[test]
    fn enforce_kill_plan_orders_phases() {
        let now = now_unix_ms().expect("now_unix_ms");

        let plan = KillPlan {
            backend: VmBackend::Docker,
            target: TargetRef {
                id: "x07-test".into(),
            },
            t_soft_unix_ms: now.saturating_sub(1_000),
            t_hard_unix_ms: now.saturating_add(50),
            t_cleanup_deadline_unix_ms: now.saturating_add(30_000),
            soft_signal: Signal::Term,
            hard_signal: Signal::Kill,
            grace: Duration::from_millis(1),
            cleanup_budget: Duration::from_millis(1),
            op_timeout: Duration::from_millis(1),
            retry: RetryPolicy {
                initial: Duration::from_millis(0),
                max: Duration::from_millis(0),
            },
        };

        let calls: RefCell<Vec<String>> = RefCell::new(Vec::new());
        let cleanup_done = RefCell::new(false);

        let res = enforce_kill_plan(
            &plan,
            &FakeBackend,
            |spec: CommandSpec| {
                calls.borrow_mut().push(spec.program.clone());
                if spec.program == "cleanup" {
                    *cleanup_done.borrow_mut() = true;
                }
                if spec.program == "probe" && *cleanup_done.borrow() {
                    return ExecResult {
                        exit_status: 1,
                        timed_out: false,
                        stdout: Vec::new(),
                        stderr: b"not found".to_vec(),
                    };
                }
                ExecResult {
                    exit_status: 0,
                    timed_out: false,
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                }
            },
            || false,
        );

        assert_eq!(res, KillResult::KilledAtHardDeadline);
        assert_eq!(
            calls.into_inner(),
            vec![
                "probe".to_string(),
                "soft".to_string(),
                "probe".to_string(),
                "hard".to_string(),
                "cleanup".to_string(),
                "probe".to_string(),
            ]
        );
    }
}
