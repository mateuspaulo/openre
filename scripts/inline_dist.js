import fs from 'fs';
import path from 'path';

const distDir = path.resolve('dist');
let html = fs.readFileSync(path.join(distDir, 'index.html'), 'utf-8');

// Inline CSS
html = html.replace(/<link\s+rel="stylesheet"\s+crossorigin\s+href="([^"]+)">/g, (match, href) => {
  const cssPath = path.join(distDir, href);
  if (fs.existsSync(cssPath)) {
    const cssContent = fs.readFileSync(cssPath, 'utf-8');
    return `<style>\n${cssContent}\n</style>`;
  }
  return match;
});

// Inline JS
html = html.replace(/<script\s+type="module"\s+crossorigin\s+src="([^"]+)">\s*<\/script>/g, (match, src) => {
  const jsPath = path.join(distDir, src);
  if (fs.existsSync(jsPath)) {
    const jsContent = fs.readFileSync(jsPath, 'utf-8');
    return `<script type="module">\n${jsContent}\n</script>`;
  }
  return match;
});

fs.writeFileSync(path.join(distDir, 'index.html'), html, 'utf-8');
console.log('Successfully inlined assets into dist/index.html! File size:', html.length, 'bytes');
