# Scenario: web-service (pure router repair)

You are given a broken X07 project snapshot intended to model a web-service handler in `solve-pure`.

Goal:

- Make `x07 fmt`, `x07 lint`, `x07 fix`, `x07 run`, and `x07 test` succeed deterministically.
- Keep the request/response logic pure (no OS imports in `solve-*` worlds).

