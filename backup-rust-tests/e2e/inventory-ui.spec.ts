import { test, expect } from '@playwright/test';

test.describe('UI Inventory', () => {
  test.beforeEach(async ({ page }) => {
    // Navigate to the app
    await page.goto('/');
    
    // Wait for the page to load
    await page.waitForLoadState('networkidle');
  });

  test('Inventory Backup Tab', async ({ page }) => {
    // Take initial screenshot
    await page.screenshot({ 
      path: 'screenshots/backup-tab-initial.png', 
      fullPage: true 
    });
    
    // Check if the page loaded correctly
    const title = await page.title();
    console.log('Page title:', title);
    
    // Count major UI elements
    const elements = {
      buttons: await page.locator('button').count(),
      inputs: await page.locator('input').count(),
      tabs: await page.locator('[role="tab"]').count(),
      progressBars: await page.locator('[role="progressbar"]').count(),
      cards: await page.locator('[data-card]').count(),
      lists: await page.locator('[role="list"]').count(),
    };
    
    console.log('Backup Tab Elements:', elements);
    
    // Test Start button
    await testButton(page, 'Start', 'screenshots/backup-after-start-click.png');
    
    // Test Pause button
    await testButton(page, 'Pause', 'screenshots/backup-after-pause-click.png');
    
    // Test Stop button  
    await testButton(page, 'Stop', 'screenshots/backup-after-stop-click.png');
  });

  test('Inventory Logs Tab', async ({ page }) => {
    // Navigate to Logs tab
    const logsTab = page.locator('[role="tab"]').filter({ hasText: 'Logs' });
    if (await logsTab.count() > 0) {
      await logsTab.click();
      await page.waitForTimeout(1000);
    }
    
    // Take screenshot
    await page.screenshot({ 
      path: 'screenshots/logs-tab-initial.png', 
      fullPage: true 
    });
    
    // Count log-related elements
    const logElements = {
      logEntries: await page.locator('[data-testid="log-entry"]').count(),
      filterButtons: await page.locator('button').filter({ hasText: /All|Errors|Warnings|Info/i }).count(),
      searchInput: await page.locator('input[type="search"]').count(),
      downloadButton: await page.locator('button').filter({ hasText: /Download/i }).count(),
    };
    
    console.log('Logs Tab Elements:', logElements);
    
    // Test filter buttons
    const filterTexts = ['All', 'Errors', 'Warnings', 'Info'];
    for (const filterText of filterTexts) {
      const filterButton = page.locator('button').filter({ hasText: new RegExp(filterText, 'i') });
      if (await filterButton.count() > 0) {
        await filterButton.click();
        await page.waitForTimeout(500);
        await page.screenshot({ 
          path: `screenshots/logs-filter-${filterText.toLowerCase()}.png`, 
          fullPage: true 
        });
      }
    }
  });

  test('Inventory Schedule Tab', async ({ page }) => {
    // Navigate to Schedule tab
    const scheduleTab = page.locator('[role="tab"]').filter({ hasText: 'Schedule' });
    if (await scheduleTab.count() > 0) {
      await scheduleTab.click();
      await page.waitForTimeout(1000);
    }
    
    // Take screenshot
    await page.screenshot({ 
      path: 'screenshots/schedule-tab-initial.png', 
      fullPage: true 
    });
    
    // Count schedule-related elements
    const scheduleElements = {
      dropdowns: await page.locator('select').count(),
      timePickers: await page.locator('input[type="time"]').count(),
      toggles: await page.locator('[role="switch"]').count(),
      checkboxes: await page.locator('input[type="checkbox"]').count(),
    };
    
    console.log('Schedule Tab Elements:', scheduleElements);
  });

  test('Inventory History Tab', async ({ page }) => {
    // Navigate to History tab
    const historyTab = page.locator('[role="tab"]').filter({ hasText: 'History' });
    if (await historyTab.count() > 0) {
      await historyTab.click();
      await page.waitForTimeout(1000);
    }
    
    // Take screenshot
    await page.screenshot({ 
      path: 'screenshots/history-tab-initial.png', 
      fullPage: true 
    });
    
    // Count history-related elements
    const historyElements = {
      historyEntries: await page.locator('[data-testid="history-entry"]').count(),
      sortButtons: await page.locator('button').filter({ hasText: /Sort/i }).count(),
      dateFilters: await page.locator('input[type="date"]').count(),
    };
    
    console.log('History Tab Elements:', historyElements);
  });
});

// Helper function to test button clicks
async function testButton(page: any, buttonText: string, screenshotPath: string) {
  const button = page.locator('button').filter({ hasText: new RegExp(buttonText, 'i') });
  if (await button.count() > 0) {
    console.log(`Testing ${buttonText} button...`);
    await button.click();
    await page.waitForTimeout(1000);
    await page.screenshot({ path: screenshotPath, fullPage: true });
  } else {
    console.log(`${buttonText} button not found`);
  }
}