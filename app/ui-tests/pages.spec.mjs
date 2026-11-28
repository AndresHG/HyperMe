// One screenshot per page per theme, compared with the approved image, and a layout check: no
// text runs past its box unless it is cut with an ellipsis, and no two pieces of text overlap.
// Run: npm test. Update the approved images after a change you mean: npm run update.
import { test, expect } from '@playwright/test';

// Text that runs out of its box or lies on other text, in the page `root`, inside the elements
// that `scope` selects (a sheet, over a page whose text it covers).
async function layoutProblems(root, scope = 'body') {
  return root.evaluate((scope) => {
    const problems = [];
    const leaves = [...document.querySelectorAll(`${scope} *`)].filter((el) => {
      if (!el.offsetParent && getComputedStyle(el).position !== 'fixed') return false;
      const own = [...el.childNodes].some((n) => n.nodeType === 3 && n.textContent.trim());
      if (!own) return false;
      const r = el.getBoundingClientRect();
      return r.width > 0 && r.height > 0 && getComputedStyle(el).visibility !== 'hidden';
    });
    for (const el of leaves) {
      const s = getComputedStyle(el);
      const cut = s.textOverflow === 'ellipsis' || s.overflow === 'hidden' || s.overflowX === 'hidden';
      if (!cut && el.scrollWidth > el.clientWidth + 1 && s.display !== 'inline') {
        problems.push(`runs past its box: "${el.textContent.trim().slice(0, 40)}"`);
      }
    }
    // Overlap of text lines, by their glyph boxes.
    const boxes = leaves.flatMap((el) => {
      const range = document.createRange();
      return [...el.childNodes].filter((n) => n.nodeType === 3 && n.textContent.trim()).flatMap((n) => {
        range.selectNodeContents(n);
        // Text cut by an ancestor (an ellipsis, a scroller) only shows inside that ancestor.
        let clip = { left: -1e9, top: -1e9, right: 1e9, bottom: 1e9 };
        for (let a = el; a && a !== document.body; a = a.parentElement) {
          if (getComputedStyle(a).overflow !== 'visible') {
            const c = a.getBoundingClientRect();
            clip = { left: Math.max(clip.left, c.left), top: Math.max(clip.top, c.top), right: Math.min(clip.right, c.right), bottom: Math.min(clip.bottom, c.bottom) };
          }
        }
        return [...range.getClientRects()].map((g) => ({
          r: { left: Math.max(g.left, clip.left), top: Math.max(g.top, clip.top), right: Math.min(g.right, clip.right), bottom: Math.min(g.bottom, clip.bottom) },
          el, text: n.textContent.trim().slice(0, 30),
        }));
      });
    });
    for (let i = 0; i < boxes.length; i++) {
      for (let j = i + 1; j < boxes.length; j++) {
        const a = boxes[i], b = boxes[j];
        if (a.el.contains(b.el) || b.el.contains(a.el)) continue;
        const x = Math.min(a.r.right, b.r.right) - Math.max(a.r.left, b.r.left);
        const y = Math.min(a.r.bottom, b.r.bottom) - Math.max(a.r.top, b.r.top);
        if (x > 2 && y > 2) problems.push(`"${a.text}" overlaps "${b.text}"`);
      }
    }
    return problems;
  }, scope);
}

async function open(page, url, size = { width: 900, height: 600 }) {
  await page.clock.setFixedTime(new Date('2026-11-27T09:30:00+01:00'));
  await page.setViewportSize(size);
  await page.goto(url);
  await page.waitForLoadState('networkidle');
}

const home = [
  ['home', 'home.html#home'],
  ['history', 'home.html#history'],
  ['dictionary', 'home.html#dictionary'],
];
for (const [name, url] of home) {
  test(name, async ({ page }) => {
    await open(page, url);
    await expect(page).toHaveScreenshot(`${name}.png`);
    expect(await layoutProblems(page)).toEqual([]);
  });
}

// Spec 267, AC-10 (R-4, R-6): Correct opens the whole text selected; Esc cancels; ⌘↵ saves, and
// the first save asks once, in a modal, before anything is kept; the row then shows its mark.
test('history-correct', async ({ page }) => {
  await open(page, 'home.html#history');
  const row = page.locator('.row[data-id="3"]');
  await row.locator('time').click();
  await page.keyboard.press('Meta+e');
  const editor = row.locator('textarea');
  await expect(editor).toBeFocused();
  expect(await editor.evaluate((t) => t.selectionEnd - t.selectionStart)).toBe('Send the weekly update to the team before lunch.'.length);
  await page.keyboard.press('Escape');
  await expect(editor).toHaveCount(0);

  await page.keyboard.press('Meta+e');
  await editor.fill('Send the weekly update to the whole team before lunch.');
  await page.keyboard.press('Meta+Enter');
  // The one-time modal (R-6): Esc closes it and keeps nothing; the next save asks again.
  const modal = page.locator('#agree');
  await expect(modal).toBeVisible();
  expect(await layoutProblems(page, '.sheet')).toEqual([]);
  await page.keyboard.press('Escape');
  await expect(modal).toHaveCount(0);
  await expect(editor).toBeVisible();
  await editor.press('Meta+Enter');
  await expect(modal).toBeVisible();
  await page.keyboard.press('Enter');
  await expect(modal).toHaveCount(0);
  await expect(row.locator('.say')).toHaveText('Saved as ground truth. Press ↵ to paste it again');
  await expect(row.locator('.truth')).toHaveText('Ground truth');
  await expect(row.locator('.text')).toHaveText('Send the weekly update to the whole team before lunch.');
});

test('settings', async ({ page }) => {
  await open(page, 'home.html#home');
  await page.evaluate(() => window.openSettings('general'));
  await expect(page.locator('#general')).toBeVisible();
  // The sidebar's words, checked as text: a short change in light grey on dark slips under the
  // screenshot's tolerance (PR 266, where settings-dark.png passed still showing "Providers").
  await expect(page.locator('.settings nav')).toHaveText(
    /Setup check\s*Everyday\s*General\s*Shortcuts\s*AI models\s*Speech to text\s*Language model\s*Setup\s*Connectors\s*Privacy\s*Diagnostics\s*About/i,
  );
  await expect(page).toHaveScreenshot('settings.png');
  expect(await layoutProblems(page, '.settings')).toEqual([]);
  // Regressions (27 November 2026): the section opened scrolled down to its anchor, and the
  // microphone rows (#221) sat outside General's grouped card.
  const layout = await page.evaluate(() => {
    const box = (el) => el.getBoundingClientRect();
    const card = box(document.querySelector('#general .card-rows'));
    const mics = [...document.querySelectorAll('#mic-list .row')].map(box);
    return {
      scrollTop: document.querySelector('.settings > main').scrollTop,
      mics: mics.length,
      inside: mics.every((m) => m.left >= card.left && m.right <= card.right && m.top >= card.top && m.bottom <= card.bottom),
    };
  });
  expect(layout).toEqual({ scrollTop: 0, mics: 2, inside: true });
});

// The AI models pages (spec 257, R-16 to R-20), after docs/ux/settings-models/.
for (const name of ['speech', 'language']) {
  test(`settings-${name}`, async ({ page }) => {
    await open(page, 'home.html#home');
    await page.evaluate((place) => window.openSettings(place), name);
    await expect(page.locator(`#${name} .model`).first()).toBeVisible();
    await expect(page).toHaveScreenshot(`settings-${name}.png`);
    expect(await layoutProblems(page, '.settings')).toEqual([]);
  });
}

test('onboarding', async ({ page }) => {
  await open(page, 'onboarding.html#keys', { width: 640, height: 620 });
  await expect(page.locator('#choices button').first()).toBeVisible();
  await expect(page).toHaveScreenshot('onboarding.png');
  expect(await layoutProblems(page)).toEqual([]);
});

// The shared controls (app/web/src/lib/ui), in a gallery built only for these checks.
for (const [name, query] of [['controls', ''], ['controls-menu', '?menu'], ['controls-sheet', '?sheet']]) {
  test(name, async ({ page }) => {
    await open(page, `gallery.html${query}`, { width: 760, height: 720 });
    await expect(page.locator('main')).toBeVisible();
    await expect(page).toHaveScreenshot(`${name}.png`);
    // The sheet is modal and covers the gallery on purpose, so only the gallery is checked.
    if (!query) expect(await layoutProblems(page)).toEqual([]);
  });
}

test('pill', async ({ page }) => {
  await open(page, 'pill.html', { width: 360, height: 300 });
  await page.evaluate(() => window.__mockEmit('card', {
    title: 'Good morning', yesterday: 'Yesterday: 4 done, 1 rolled over', meetings: '3 meetings · first at 09:30',
    tasks: [
      { title: 'Review the tokens pull request', tone: 'due', mark: 'due today' },
      { title: 'Send the weekly update', tone: 'unblocked', mark: 'unblocked' },
      { title: 'Book the offsite room', tone: null, mark: null },
    ],
    footer: null, mail: null, stale: null,
  }));
  await expect(page).toHaveScreenshot('pill-card.png');
  expect(await layoutProblems(page)).toEqual([]);
  await page.setViewportSize({ width: 200, height: 32 });
  await page.evaluate(() => window.__mockEmit('pill', { kind: 'done', label: 'Pasted · 42 words' }));
  await expect(page).toHaveScreenshot('pill.png');
});
