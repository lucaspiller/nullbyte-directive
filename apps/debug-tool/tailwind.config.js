/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{svelte,js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        'terminal-bg': '#0f172a', // Slate 900
        'terminal-fg': '#cbd5e1', // Slate 300
        'accent-primary': '#4ade80', // Green 400
        'accent-warning': '#fbbf24', // Amber 400
        'accent-error': '#f87171', // Red 400
        'panel-bg': '#1e293b', // Slate 800
        'panel-border': '#334155', // Slate 700
      },
      fontFamily: {
        mono: ['Fira Code', 'Menlo', 'Monaco', 'Courier New', 'monospace'],
      },
    },
  },
  plugins: [],
}
