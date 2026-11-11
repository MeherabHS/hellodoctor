import tseslint from 'typescript-eslint';

export default tseslint.config(
  { ignores: ['.next/**', 'node_modules/**', 'next-env.d.ts'] },
  tseslint.configs.base,
  {
    files: ['src/**/*.{ts,tsx}', 'tests/**/*.ts'],
    rules: {
      // TypeScript checks names and types. These checks add control-flow and
      // async correctness without relaxing the existing source-safety gate.
      'no-async-promise-executor': 'error',
      'no-duplicate-case': 'error',
      'no-unsafe-finally': 'error',
      'no-unsafe-optional-chaining': 'error',
      'valid-typeof': 'error',
      'use-isnan': 'error',
      '@typescript-eslint/no-duplicate-enum-values': 'error',
      '@typescript-eslint/no-non-null-asserted-optional-chain': 'error',
    },
  },
);
