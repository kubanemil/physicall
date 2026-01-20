# physicall-web

TypeScript frontend for Physicall using Vite.

## Overview

Minimal web shell that:
- Loads the WASM module
- Creates a fullscreen canvas
- Runs the render loop
- Handles keyboard events (camera reset)

## Setup

```bash
# Install dependencies
npm install

# Start dev server
npm run dev

# Build for production
npm run build
```

## Scripts

- `npm run dev` - Start development server with hot reload
- `npm run build` - TypeScript check + production build
- `npm run preview` - Preview production build

## Dependencies

- **Vite** - Build tool and dev server
- **TypeScript** - Type checking
- **vite-plugin-wasm** - WASM support
- **vite-plugin-top-level-await** - Top-level await in modules
- **@webgpu/types** - WebGPU TypeScript definitions

## Browser Requirements

WebGPU-compatible browser required:
- Chrome 113+
- Firefox 121+
- Safari 18+

## Project Structure

```
web/
├── index.html       # Entry HTML with canvas
├── src/
│   ├── main.ts      # WASM init + render loop
│   └── style.css    # Fullscreen canvas styles
├── vite.config.ts   # Vite configuration
└── tsconfig.json    # TypeScript configuration
```
