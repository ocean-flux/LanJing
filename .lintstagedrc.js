export default {
  'src/**/*.{svelte,ts,js,css,html,json}': ['eslint --fix', 'prettier --write'],
  '*.{json,js,mjs,ts,html}': ['prettier --write'],
  // cargo fmt does not accept file paths like prettier; format whole workspace crate tree
  'src-tauri/**/*.rs': () => 'cargo fmt --manifest-path src-tauri/Cargo.toml',
};
