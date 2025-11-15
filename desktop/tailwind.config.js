/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./src/**/*.{html,js,svelte,ts}",
  ],
  theme: {
    extend: {
      colors: {
        discord: {
          blurple: '#5865F2',
          green: '#57F287',
          yellow: '#FEE75C',
          fuchsia: '#EB459E',
          red: '#ED4245',
          white: '#FFFFFF',
          black: '#23272A',
          dark: '#2C2F33',
          darker: '#1E2124',
        }
      }
    },
  },
  plugins: [],
}