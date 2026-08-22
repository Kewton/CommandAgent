# Implementation Summary: Issue #255

Implemented named-preset composition and discovery as part of the combined #255/#229/#232 contract.

- Added recursive, single-parent `extends` resolution. Child values override inherited values, inherited fields retain their originating preset in source metadata, and missing parents or cycles fail with actionable configuration errors.
- Added `${ENV_NAME}` expansion for quoted configuration values, including quoted numeric fields. Missing and non-Unicode values fail configuration resolution and therefore surface as failed `--doctor` configuration checks without printing secret contents.
- Added offline preset-name discovery across the established configuration search paths and connected it to dynamic `--preset` shell completion with prefix filtering, sorting, and deduplication.
- Updated the English and Japanese configuration guides and README examples for inheritance, environment references, and the expanded supported-key contract.
- Added focused unit and corpus coverage for inheritance, cycles, environment resolution, doctor failure projection, and completion discovery.

The implementation preserves the existing per-preset cross-file precedence and early-stop rules.
