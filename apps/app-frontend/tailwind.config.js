/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{vue,ts}"],
  theme: {
    extend: {
      colors: {
        bg: "#0e1014",
        panel: "#15181e",
        panel2: "#1b1f27",
        text: "#e8eaf0",
        acc: "#ff8fb8",
      },
      fontFamily: {
        sans: ["Prompt", "system-ui", "sans-serif"],
        mono: ["IBM Plex Mono", "ui-monospace", "monospace"],
      },
    },
  },
  plugins: [],
};
