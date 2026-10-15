# add-routing-mode tasks

- [x] 1. Red: e2e tests for declarative routing mode (resolve, append, validation, private override, init layer)
- [x] 2. `Routing` enum + `routing` key parsing/validation in repo config
- [x] 3. `resolve()` honors the mode for repo/branch scopes
- [x] 4. `init` provisions only the declared layer
- [x] 5. doctor `turu.routing` check (effective route + stale-layer warn)
- [x] 6. Green: full suite + `just ci`
- [x] 7. Deploy spec deltas, bind .espectacular contracts, `ah check` green
