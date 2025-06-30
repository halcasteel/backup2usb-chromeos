const { chromium } = require('playwright');

(async () => {
  console.log('Starting browser...');
  const browser = await chromium.launch({ headless: true });
  const page = await browser.newPage();
  
  try {
    console.log('Navigating to localhost:3000...');
    await page.goto('http://localhost:3000', { waitUntil: 'networkidle' });
    
    // Wait for the app to load
    await page.waitForTimeout(2000);
    
    // Take screenshot of initial state
    await page.screenshot({ path: 'screenshots/01-initial-state.png', fullPage: true });
    console.log('Screenshot 1: Initial state captured');
    
    // Click on Logs tab
    const logsTab = page.locator('button:has-text("Logs")').first();
    if (await logsTab.count() > 0) {
      await logsTab.click();
      console.log('Clicked on Logs tab');
      await page.waitForTimeout(1000);
    } else {
      // Try alternative selector
      const altLogsTab = page.locator('[role="tab"]:has-text("Logs")').first();
      if (await altLogsTab.count() > 0) {
        await altLogsTab.click();
        console.log('Clicked on Logs tab (alt selector)');
        await page.waitForTimeout(1000);
      }
    }
    
    // Take screenshot of logs tab
    await page.screenshot({ path: 'screenshots/02-logs-tab.png', fullPage: true });
    console.log('Screenshot 2: Logs tab captured');
    
    // Check what's in the logs area
    const logEntries = await page.locator('[data-testid="log-entry"]').count();
    console.log(`Found ${logEntries} log entries`);
    
    // Check for empty state message
    const emptyState = await page.locator('text=/no logs|empty/i').count();
    if (emptyState > 0) {
      console.log('Empty state message found');
    }
    
    // Get any text content in the logs area
    const logsContent = await page.locator('[role="tabpanel"]').textContent();
    console.log('Logs tab content:', logsContent?.substring(0, 200));
    
  } catch (error) {
    console.error('Error:', error);
    await page.screenshot({ path: 'screenshots/error-state.png', fullPage: true });
  } finally {
    await browser.close();
    console.log('Browser closed');
  }
})();