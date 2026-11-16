Time profile: provider 100% · installs 0% · builds 0% · probe 0% · total 3s

Provider usage by role:
| Role | Provider time | Prompt tokens | Generation tokens | Thinking tokens | Prefill ratio |
| --- | ---: | ---: | ---: | ---: | ---: |
| executor | 3s | 300 | 60 | n/a | n/a |

Structured stages: incomplete (1/2 sealed)
| Stage | Target | Phase | Attempts | Failed checks |
| --- | --- | --- | ---: | --- |
| alpha | main | sealed | 1/8 | - |
| beta | main | proposed | 2/8 | attempt 2: schema=passed; missing lint; 1 unrequested |

Time profile by phase:
| Phase | Total | Provider | Installs | Builds | Probe | Other |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| unscoped | 3s | 3s | 0s | 0s | 0s | 0s |

Generation profile (duration-weighted eval tokens):
| Caller scope | Eval tokens | Duration | Turns |
| --- | ---: | ---: | ---: |
| executor | 60 | 3s | 1 |

| Turn type | Eval tokens | Duration | Turns | Write bytes | Edit bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| prose-only | 60 | 3s | 1 | 0B | 0B |
