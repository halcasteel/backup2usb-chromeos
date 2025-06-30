import { test, expect } from '@playwright/test';

test.describe('API Health Checks', () => {
  test('Detect hanging API calls', async ({ page }) => {
    let apiHanging = false;
    let apiTimeouts = [];
    let apiErrors = [];
    
    // Track API calls and timeouts
    page.on('response', response => {
      const url = response.url();
      if (url.includes('/api/status')) {
        console.log(`API Response: ${url} - Status: ${response.status()}`);
        if (response.status() === 0) {
          apiHanging = true;
          apiErrors.push(`${url} - Status: 0 (hanging)`);
        }
      }
    });
    
    page.on('requestfailed', request => {
      const url = request.url();
      if (url.includes('/api/')) {
        console.log(`API Request Failed: ${url} - ${request.failure()?.errorText}`);
        apiErrors.push(`${url} - ${request.failure()?.errorText}`);
      }
    });
    
    // Set a timeout for the entire test
    test.setTimeout(15000);
    
    try {
      console.log('Navigating to app...');
      await page.goto('/', { waitUntil: 'networkidle', timeout: 10000 });
      
      console.log('Waiting for API calls to complete...');
      await page.waitForTimeout(5000);
      
      // Try to manually trigger an API call
      console.log('Attempting to fetch /api/status directly...');
      const response = await page.evaluate(async () => {
        try {
          const response = await fetch('/api/status', { 
            method: 'GET',
            headers: { 'Content-Type': 'application/json' }
          });
          return {
            status: response.status,
            ok: response.ok,
            statusText: response.statusText,
            body: response.ok ? await response.text() : null
          };
        } catch (error) {
          return {
            error: error.message,
            status: 0
          };
        }
      });
      
      console.log('API Status Response:', response);
      
      if (response.error) {
        apiErrors.push(`Direct fetch error: ${response.error}`);
      }
      
    } catch (error) {
      console.error('Test error:', error);
      apiErrors.push(`Test error: ${error.message}`);
    }
    
    // Log all findings
    console.log('API Hanging:', apiHanging);
    console.log('API Errors:', apiErrors);
    
    // Take screenshot of current state
    await page.screenshot({ 
      path: 'screenshots/api-health-check.png', 
      fullPage: true 
    });
    
    // Fail the test if there are issues
    if (apiHanging || apiErrors.length > 0) {
      throw new Error(`API Issues detected: ${JSON.stringify({ apiHanging, apiErrors }, null, 2)}`);
    }
  });

  test('Detect WebSocket failures', async ({ page }) => {
    const wsErrors = [];
    const wsMessages = [];
    
    page.on('console', msg => {
      const text = msg.text();
      console.log(`Console ${msg.type()}: ${text}`);
      
      if (text.includes('WebSocket') && msg.type() === 'error') {
        wsErrors.push(text);
      }
      if (text.includes('WebSocket')) {
        wsMessages.push(`${msg.type()}: ${text}`);
      }
    });
    
    try {
      await page.goto('/', { waitUntil: 'networkidle', timeout: 10000 });
      await page.waitForTimeout(3000);
      
      console.log('WebSocket Messages:', wsMessages);
      console.log('WebSocket Errors:', wsErrors);
      
      // Take screenshot
      await page.screenshot({ 
        path: 'screenshots/websocket-health-check.png', 
        fullPage: true 
      });
      
      if (wsErrors.length > 0) {
        throw new Error(`WebSocket Issues detected: ${JSON.stringify(wsErrors, null, 2)}`);
      }
      
    } catch (error) {
      console.error('WebSocket test error:', error);
      throw error;
    }
  });

  test('Check backend server health', async ({ page }) => {
    // Test if backend is responsive
    const backendHealth = await page.evaluate(async () => {
      try {
        const response = await fetch('http://localhost:8888/api/status', {
          method: 'GET',
          headers: { 'Content-Type': 'application/json' },
          signal: AbortSignal.timeout(5000) // 5 second timeout
        });
        return {
          status: response.status,
          ok: response.ok,
          headers: Object.fromEntries(response.headers.entries())
        };
      } catch (error) {
        return {
          error: error.message,
          status: 0
        };
      }
    });
    
    console.log('Backend Health:', backendHealth);
    
    if (backendHealth.error || backendHealth.status === 0) {
      throw new Error(`Backend not healthy: ${JSON.stringify(backendHealth)}`);
    }
    
    expect(backendHealth.status).toBe(200);
  });
});