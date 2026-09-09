import js from '@eslint/js';
import babelParser from '@babel/eslint-parser';
import nextPlugin from '@next/eslint-plugin-next';
import reactHooks from 'eslint-plugin-react-hooks';
import prettierConfig from 'eslint-config-prettier';
import globals from 'globals';

export default [
  js.configs.recommended,
  {
    files: ['**/*.{js,mjs,ts,tsx}'],
    languageOptions: {
      parser: babelParser,
      parserOptions: {
        requireConfigFile: false,
        babelOptions: {
          presets: ['@babel/preset-typescript'],
          plugins: ['@babel/plugin-syntax-jsx'],
        },
      },
      globals: {
        ...globals.browser,
        ...globals.node,
      },
    },
    plugins: {
      '@next/next': nextPlugin,
      'react-hooks': reactHooks,
    },
    rules: {
      ...nextPlugin.configs.recommended.rules,
      'react-hooks/exhaustive-deps': 'warn',
      // Babel parses TypeScript syntax, but the core ESLint variable rules do
      // not understand type-only declarations. `tsc` remains authoritative
      // for type and symbol checking.
      'no-undef': 'off',
      'no-unused-vars': 'off',
    },
  },
  prettierConfig,
  {
    ignores: ['.next/**', 'dist/**', 'node_modules/**'],
  },
];
