// Renders the README images for HyperMe: wordmark logos (light, dark), the demo placeholder,
// the social preview and the two web icons. Usage: node render-readme-images.js <out-dir>
// Needs Playwright with Chromium and the Inter font installed. See docs/brand/README.md.
const { chromium } = require('playwright');
const fs = require('fs');
const icon = (dark) => `
<svg xmlns="http://www.w3.org/2000/svg" viewBox="100 100 824 824" width="%S" height="%S">
<defs><linearGradient id="g${dark?'d':'l'}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${dark?'#26262B':'#9077FF'}"/><stop offset="1" stop-color="${dark?'#111113':'#6440F2'}"/></linearGradient></defs>
<rect x="102" y="102" width="820" height="820" rx="185" fill="url(#g${dark?'d':'l'})" ${dark?'stroke="#3A3A42" stroke-width="4"':''}/>
<rect x="250" y="440" width="120" height="56" rx="28" fill="#FFFFFF" opacity=".4"/><rect x="300" y="528" width="90" height="56" rx="28" fill="#FFFFFF" opacity=".25"/><rect x="420" y="404" width="354" height="216" rx="108" fill="#FFFFFF"/><circle cx="666" cy="512" r="48" fill="${dark?'#8F74FF':'#6440F2'}"/></svg>`;
const base = `<style>*{margin:0;padding:0;box-sizing:border-box}body{background:transparent;font-family:'Inter Display','Inter',sans-serif;-webkit-font-smoothing:antialiased}</style>`;
const lockup = (dark) => `${base}<div id="x" style="display:inline-flex;align-items:center;gap:22px;padding:8px 12px">
${icon(dark).replace(/%S/g,'104')}
<span style="font-size:76px;font-weight:700;letter-spacing:-2.2px;color:${dark?'#F4F4F6':'#16161A'}">Hyper<span style="color:${dark?'#9C86FF':'#6440F2'}">Me</span></span></div>`;
const bars = (n, seed) => Array.from({length:n},(_,i)=>{const h=[8,14,20,26,18,24,12,22,16,10,6][(i+seed)%11];return `<i style="display:block;width:4px;height:${h}px;border-radius:2px;background:#fff"></i>`}).join('');
const pill = (inner, w) => `<div style="height:48px;min-width:${w}px;padding:0 22px;border-radius:24px;background:#0E0E11;border:1px solid #2C2C33;box-shadow:0 10px 30px rgba(0,0,0,.35);display:flex;align-items:center;gap:12px;color:#fff;font-size:17px;font-weight:600">${inner}</div>`;
const step = (label, sub, p) => `<div style="display:flex;flex-direction:column;align-items:center;gap:18px"><div style="height:56px;display:flex;align-items:center">${p}</div><div style="text-align:center"><div style="color:#F4F4F6;font-size:17px;font-weight:600">${label}</div><div style="color:#9A9AA6;font-size:14px;margin-top:4px">${sub}</div></div></div>`;
const arrow = `<div style="color:#5A5A66;font-size:26px;margin-top:-44px">&rarr;</div>`;
const demo = `${base}<div id="x" style="width:960px;padding:44px 40px 30px;border-radius:20px;background:linear-gradient(180deg,#1C1C22,#121216);border:1px solid #2A2A31">
<div style="display:flex;align-items:center;justify-content:center;gap:34px">
${step('Hold a key and talk','in any app', pill(`<i style="display:block;width:10px;height:10px;border-radius:5px;background:#FF4D4D"></i><span style="display:flex;align-items:center;gap:4px">${bars(11,0)}</span>`,150))}
${arrow}
${step('Let go','speech and cleanup run', pill(`<span style="display:block;width:110px;height:2px;background:linear-gradient(90deg,transparent,#3A3A44 20%,#9C86FF 55%,#3A3A44 80%,transparent);box-shadow:0 0 12px #7C5CFF"></span>`,150))}
${arrow}
${step('Clean text at your cursor','English or Spanish', pill(`<i style="display:block;width:14px;height:3px;border-radius:2px;background:#34D058"></i><span>Pasted</span>`,150))}
</div>
<div style="margin-top:30px;text-align:center;color:#6E6E7A;font-size:13px;letter-spacing:.2px">Mockup of the dictation pill. A screen recording replaces it once the app is ready to try.</div></div>`;
const social = `${base}<div id="x" style="width:1280px;height:640px;background:radial-gradient(1000px 500px at 50% 0%,#2A2140 0%,#141418 60%);display:flex;flex-direction:column;align-items:center;justify-content:center;gap:34px">
<div style="display:flex;align-items:center;gap:30px">${icon(false).replace(/%S/g,'150')}<span style="font-size:112px;font-weight:700;letter-spacing:-3.4px;color:#F4F4F6">Hyper<span style="color:#9C86FF">Me</span></span></div>
<div style="font-size:40px;font-weight:500;color:#D6D6DE;letter-spacing:-.6px">The open-source voice copilot for your workday</div>
<div style="font-size:24px;color:#8C8C98">Starts with dictation · Mac first · your own keys · MIT OR Apache-2.0</div></div>`;
(async()=>{
  const b = await chromium.launch(); const p = await b.newPage({deviceScaleFactor:2});
  const out = process.argv[2];
  for (const [name, html, transparent] of [['hyperme-logo-light',lockup(false),true],['hyperme-logo-dark',lockup(true),true],['demo-placeholder',demo,true],['social-preview',social,false]]) {
    await p.setContent(html); await p.evaluate(()=>document.fonts.ready);
    if (name==='social-preview') { await p.setViewportSize({width:1280,height:640}); const p1 = await b.newPage({deviceScaleFactor:1,viewport:{width:1280,height:640}}); await p1.setContent(html); await p1.evaluate(()=>document.fonts.ready); await p1.screenshot({path:`${out}/${name}.png`}); await p1.close(); continue; }
    await p.locator('#x').screenshot({path:`${out}/${name}.png`, omitBackground:transparent});
  }
  fs.writeFileSync(`${out}/hyperme-icon-light.svg`, icon(false).replace(/%S/g,'512').trim()+'\n');
  fs.writeFileSync(`${out}/hyperme-icon-dark.svg`, icon(true).replace(/%S/g,'512').trim()+'\n');
  await b.close();
})();
