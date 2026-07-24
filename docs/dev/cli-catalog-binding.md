# CLI catalog binding plan (E-3a draft)

| Evidence | Existing component | Status |
|---|---|---|
| C1 normal/error execution | pipeline/probe execution boundary | ✅ reuse |
| C2 help binding | help/interface comparator | 🟡 new, ~90 Rust LOC |
| C3 output claims | claims-binding comparator | ✅ reuse |
| C4 rerun consistency | rerun consistency evidence | ✅ reuse |

The only anticipated new Rust pieces are the help-binding comparator and an
argv probe adapter: **2 components, approximately 180 new Rust lines total**.
This is a forecast for the E-3 wager, not an implementation commitment. The
catalog does not yet admit the profile; fix behavior remains deferred for
review.
