# Escaped importer regression

Parent proposed `import { atomic\u0057rite as write } from '@/lib/store'` while
an ordinary tasks route still imports store. The committed fixture is
`tests/corpus/apps/issue475-store-preflight/escaped-import-route.ts`.
`node --check` on Node v24.1.0 accepts its syntax.

The focused test calls the actual product import closure and observation policy.
It asserts that `src/lib/store.ts` remains in the closure and the escaped route
is outside the small Source parser's language. With the new generic-recognition
leaf's escape guard removed, the test fails:

```
ISSUE475_ESCAPED_IMPORT allowed_generated_paths=["data/projects.json", "data/tasks.json"]
```

This was reproduced locally after the parent requested the counterexample; it
was not a pre-existing grant in the original e43b2d86 implementation, which did
not recognize this generic writer at all. Restore the guard and the exact same
product-policy test passes with `allowed_generated_paths=[]`.

The final fix refuses only the new generic grants when another closure module
contains Unicode escape syntax. It deliberately does not decode or partially
trust escaped names. Names inside comments/strings can cause conservative false
negatives. The all-module scan also rejects namespace/dynamic imports and opaque
evaluation; direct writer recognition retains its existing behavior.

The focused final suite includes this test, the alternate escaped spelling,
normal/aliased/namespace/dynamic external references, mixed known/dynamic calls,
reassignment/shadowing, recursion and a nine-boundary refusal. No source/hash or
acceptance gate was relaxed to make the counterexample pass.
