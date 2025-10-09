/** @type {import('tailwindcss').Config} */
export default {
  content: [
    "./index.html",
    "./src/**/*.{js,ts,jsx,tsx}",
  ],
  theme: {
    extend: {
      colors: {
        primary: {
          50: '#f5f7ff',
          100: '#ebf0ff',
          500: '#667eea',
          600: '#5568d3',
          700: '#4451b8',
          800: '#764ba2',
        },
      },
    },
  },
  plugins: [],
}

