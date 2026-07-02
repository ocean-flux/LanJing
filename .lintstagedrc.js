export default {
  'src/**/*.{svelte,ts,js,css,html,json}': ['eslint --fix', 'prettier --write'],
  '*.{json,js,mjs,ts,html}': ['prettier --write'],
  '*.rs': ['cargo fmt'],
};
