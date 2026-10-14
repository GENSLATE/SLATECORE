# @genslate/config-typescript

Shared, strict TypeScript 7 configurations for SLATECORE packages and apps.

| File | Use |
|---|---|
| `tsconfig.base.json` | Strictest base: `moduleResolution: bundler`, `verbatimModuleSyntax`, `noUncheckedIndexedAccess`, `exactOptionalPropertyTypes`, `noEmit`. |
| `tsconfig.bun.json` | Bun scripts and tooling (`types: ["bun"]`). |
| `tsconfig.react.json` | React 19 DOM code (`jsx: react-jsx`, DOM libs). |

```json
{ "extends": "@genslate/config-typescript/tsconfig.react.json", "include": ["src", "tests"] }
```

TypeScript 7 is the native compiler: `baseUrl`, `moduleResolution: node10`, `outFile` and `target: ES5` are hard errors.
