/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{ts,tsx}"],
  theme: {
    extend: {
      colors: {
        // Razer signature green used for accents and glows
        "razer-green": "#44d62c",
      },
    },
  },
  plugins: [],
};
