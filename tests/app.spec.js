import { expect, test } from "@playwright/test";

test("app shell loads with fixed player outside scrollable content", async ({ page }) => {
  await page.goto("/");

  await expect(page).toHaveTitle("Yolite");
  await expect(page.getByRole("heading", { name: "Home" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Play", exact: true })).toBeVisible();

  const layout = await page.evaluate(() => {
    const rect = (selector) => {
      const box = document.querySelector(selector)?.getBoundingClientRect();
      if (!box) return null;
      return {
        bottom: Math.round(box.bottom),
        height: Math.round(box.height),
        top: Math.round(box.top),
      };
    };

    return {
      viewportHeight: window.innerHeight,
      documentHeight: document.documentElement.scrollHeight,
      shell: rect(".shell"),
      main: rect(".main"),
      player: rect(".player"),
    };
  });

  expect(layout.player.top).toBe(layout.shell.bottom);
  expect(layout.main.bottom).toBe(layout.player.top);
  expect(layout.documentHeight).toBe(layout.viewportHeight);
});

test("discover uses expanded 12-card desktop shelves", async ({ page }) => {
  await page.goto("/");
  await page.getByRole("button", { name: "Discover" }).click();
  await expect(page.locator("#discoverPanel.panel.active")).toBeVisible();
  await expect(page.locator(".discover-grid").first()).toBeVisible();

  const shelf = await page.locator(".discover-grid").first().evaluate((grid) => ({
    cardCount: grid.children.length,
    columns: getComputedStyle(grid).gridTemplateColumns.split(" ").length,
  }));

  expect(shelf.cardCount).toBeLessThanOrEqual(12);
  expect(shelf.columns).toBe(3);
});

test("playlist opens cached tracks immediately then refreshes", async ({ page }) => {
  await page.route("**/api/library", async (route) => {
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        needsLogin: false,
        sections: [],
        playlists: [{ id: "PLcached", title: "Cached playlist", thumbnail: "" }],
      }),
    });
  });
  await page.route("**/api/playlist/PLcached", async (route) => {
    await page.waitForTimeout(300);
    await route.fulfill({
      contentType: "application/json",
      body: JSON.stringify({
        tracks: [
          { id: "fresh1", title: "Fresh track", artist: "Network" },
          { id: "cached1", title: "Cached track", artist: "Local" },
        ],
      }),
    });
  });

  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem("yolite:playlist:PLcached:tracks", JSON.stringify({
      savedAt: Date.now(),
      tracks: [{ id: "cached1", title: "Cached track", artist: "Local" }],
    }));
  });
  await page.reload();
  await page.getByRole("button", { name: "Library" }).click();
  await page.locator("#library").getByRole("button", { name: "Cached playlist" }).click();

  await expect(page.getByText("Cached track")).toBeVisible();
  await expect(page.getByText("Fresh track")).toBeVisible();
  await expect(page.locator("#status")).toContainText("Playlist updated");
});

test("history keeps 50 plays and loop cycles through all modes", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    const plays = Array.from({ length: 55 }, (_, index) => ({
      id: `history-${index}`,
      title: `History song ${index}`,
      artist: "Yolite test",
      album: "Recent plays",
      duration: 180,
      playedAt: Date.now() - index,
    }));
    localStorage.setItem("yolite:recentPlays", JSON.stringify(plays));
  });
  await page.reload();
  await page.getByRole("button", { name: "History" }).click();
  await expect(page.locator("#history .track")).toHaveCount(50);

  const loop = page.locator("#loopBtn");
  await expect(loop).toHaveAttribute("aria-label", "Loop off");
  await loop.click();
  await expect(loop).toHaveAttribute("aria-label", "Loop playlist");
  await loop.click();
  await expect(loop).toHaveAttribute("aria-label", "Loop song");
  await loop.click();
  await expect(loop).toHaveAttribute("aria-label", "Loop off");
});

test("track context menu exposes mix, play-next, and playlist actions", async ({ page }) => {
  await page.goto("/");
  await page.evaluate(() => {
    localStorage.setItem("yolite:recentPlays", JSON.stringify([{
      id: "context-track",
      title: "Context track",
      artist: "Yolite test",
      album: "Interactions",
      duration: 180,
    }]));
  });
  await page.reload();
  await page.getByRole("button", { name: "History" }).click();
  await page.locator("#history .track").click({ button: "right" });
  await expect(page.getByRole("button", { name: "Start mix" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Play after this song" })).toBeVisible();
  await expect(page.getByRole("button", { name: "Add to playlist" })).toBeVisible();
});

test("Listen again renders three animated 3x3 pages", async ({ page }) => {
  const tracks = Array.from({ length: 27 }, (_, index) => ({
    id: `listen-${index}`,
    title: `Listen track ${index}`,
    artist: "Yolite test",
    album: "Listen again",
    duration: 180,
    thumbnail: "",
  }));
  await page.route("**/api/home", route => route.fulfill({
    contentType: "application/json",
    body: JSON.stringify({ sections: [{ title: "Listen again", tracks, layout: "grid" }] }),
  }));
  await page.goto("/");
  await expect(page.locator(".listen-page")).toHaveCount(3);
  await expect(page.locator(".listen-page").first().locator(".track-card")).toHaveCount(9);
  const pages = page.locator(".listen-pages");
  await expect(pages).toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");
  await page.getByRole("button", { name: "Next Listen again page" }).click();
  await expect(pages).not.toHaveCSS("transform", "matrix(1, 0, 0, 1, 0, 0)");
});

test("settings owns session and performance controls", async ({ page }) => {
  await page.goto("/");

  await expect(page.getByRole("button", { name: "Session" })).toHaveCount(0);
  await page.getByRole("button", { name: "Settings" }).click();
  await expect(page.getByRole("heading", { name: "Performance" })).toBeVisible();
  await expect(page.getByRole("heading", { name: "Session" })).toBeVisible();
  await expect(page.locator("#prefetchCount")).toBeVisible();
});
