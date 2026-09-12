// Executable analogue of a build wrapper whose new configuration can skip checks.
// The Runner regression must reject this changed condition even when it exits 0.
import { existsSync } from 'node:fs';

const config = existsSync('next.config.js')
  ? (await import('../next.config.js')).default
  : {};
if (config.typescript?.ignoreBuildErrors) {
  console.log('Skipping type checking');
} else {
  await import('./target.js');
}
