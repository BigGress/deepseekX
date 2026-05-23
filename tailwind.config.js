/** @type {import('tailwindcss').Config} */
export default {
  content: ["./index.html", "./src/**/*.{js,ts,jsx,tsx}"],
  theme: {
    extend: {
      colors: {
        neutral: {
          850: "#1a1a1a",
          925: "#0f0f0f",
        },
      },
    },
  },
  plugins: [],
};
