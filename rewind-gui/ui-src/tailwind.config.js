module.exports = {
  content: ['./app.jsx', '../ui/index.html'],
  theme: {
    extend: {
      colors: {
        ink: { 950: '#0c0c11', 900: '#111117', 850: '#15151c', 800: '#1a1a23', 700: '#262630', 600: '#33333f' }
      },
      fontFamily: {
        sans: ['Inter', 'Segoe UI Variable', 'Segoe UI', 'system-ui', 'sans-serif'],
        mono: ['JetBrains Mono', 'Cascadia Mono', 'Consolas', 'monospace']
      }
    }
  }
};
