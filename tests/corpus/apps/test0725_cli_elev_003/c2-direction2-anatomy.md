# C2 direction-2 measured anatomy

Source: `uat-test0725-cli-elev-003`, `filter_cloud_002`,
`evidence/help-binding.json`.

Measured probe:

```json
{"direction":"implementation_to_help","option":"--anvil-invalid-probe","args":["--anvil-invalid-probe"],"exit_code":2,"stderr":"main.py: error: the following arguments are required: file, --pattern","ok":false,"nearest_miss":{"candidate":"--pattern","edit_distance":17}}
```

The CLI did reject the process, but argparse adjudicated missing required
arguments before it reached unknown-option reporting. C2 v0 required an
`unrecognized`-family observation, so the comparator correctly rejected the
observation it was given. The defect is the machine-owned probe shape: it
injected the invalid option alone instead of adding it to the frozen,
executable normal argv.

CLI-4 keeps the comparator strict and changes direction 2 to append
`--anvil-invalid-probe` to the frozen normal argv.
