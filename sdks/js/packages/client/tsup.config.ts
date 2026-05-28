import { defineConfig } from 'tsup';

export default defineConfig({
  entry: ['src/index.ts'],
  format: ['esm', 'cjs'],
  dts: true,
  sourcemap: true,
  clean: true,
  treeshake: true,
  target: 'es2022',
  external: [
    '@reactor/shared',
    '@reactor/auth',
    '@reactor/data',
    '@reactor/storage',
    '@reactor/functions',
    '@reactor/jobs',
    '@reactor/sites',
    '@reactor/realtime',
  ],
});
