# Support triage contract

This local mock project demonstrates provider rate limiting followed by safe and unsafe backup behavior.
The customer request and all provider responses are synthetic examples.

```bash
llm-fallback-compatibility-tester validate --config contract.yaml
llm-fallback-compatibility-tester run --config contract.yaml --output runs/demo
```

No API key or model is needed. The unsafe scenario is expected to fail its critical application assertions, so the command exits with status 1 after writing the reports.
