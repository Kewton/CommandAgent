// Deliberately unsupported syntax: this whole module grants no outputs.
import { writeFile } from 'node:fs/promises';
const regex = /writeFile("data\/regex.json", "[]")/;
const template = `writeFile('data/template.json', '[]')`;
const interpolated = `writer ${writeFile('data/interpolation.json', '[]')}`;
export { regex, template, interpolated };
