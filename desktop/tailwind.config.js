/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./src/**/*.{html,js,svelte,ts}",
  ],
  theme: {
    extend: {
      colors: {
        discord: {
          primary: "#5030E5",
          blurple: '#5865F2',
          green: '#57F287',
          yellow: '#FBB13C',
          fuchsia: '#EB459E',
          red: '#FB2C2C',
          white: '#FFFFFF',
          grey: "#EBEBEB",
          "dark-grey": "#949494",
          black: '#23272A',
          dark: '#2C2F33',
          darker: '#1E2124',
        }
      }
    },
  },
  plugins: [],
}