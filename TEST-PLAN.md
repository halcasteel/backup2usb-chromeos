# 📋 Master Testing & Fix Checklist

## Phase 1: Environment Setup ✅

### Testing Infrastructure
- [ ] Create test directory structure
  ```
  backup-rust-tests/
  ├── e2e/
  │   ├── playwright/
  │   ├── screenshots/
  │   └── reports/
  ├── integration/
  ├── unit/
  └── fixtures/
  ```
- [ ] Install Playwright: `npm install --save-dev @playwright/test playwright`
- [ ] Install Playwright browsers: `npx playwright install chromium`
- [ ] Create `playwright.config.ts`
- [ ] Create `e2e/` directory
- [ ] Create `screenshots/` directory
- [ ] Create `reports/` directory
- [ ] Set up test data fixtures

### Test Utilities
- [ ] Create screenshot helper functions
- [ ] Create API mock server
- [ ] Create test data generators
- [ ] Set up HTML report generation
- [ ] Configure test timeouts

## Phase 2: Current State Documentation 📸

### Backup Tab Inventory
- [ ] Navigate to Backup tab
- [ ] Screenshot initial state
- [ ] Count directory items
- [ ] Count control buttons
- [ ] Count progress indicators
- [ ] Test Start button - take screenshot
- [ ] Test Pause button - take screenshot
- [ ] Test Stop button - take screenshot
- [ ] Document any errors in console

### Logs Tab Inventory
- [ ] Navigate to Logs tab
- [ ] Screenshot initial state
- [ ] Count log entries (should be 0 or more)
- [ ] Test "All" filter - screenshot
- [ ] Test "Errors" filter - screenshot
- [ ] Test "Warnings" filter - screenshot
- [ ] Test "Info" filter - screenshot
- [ ] Test search functionality - screenshot
- [ ] Test log download button
- [ ] Document missing features

### Schedule Tab Inventory
- [ ] Navigate to Schedule tab
- [ ] Screenshot initial state
- [ ] Test frequency dropdown
- [ ] Test time picker
- [ ] Test profile selection
- [ ] Test enable/disable toggle
- [ ] Document any non-functional elements

### History Tab Inventory
- [ ] Navigate to History tab
- [ ] Screenshot initial state
- [ ] Count history entries
- [ ] Test sorting options
- [ ] Test date filters
- [ ] Document display issues

## Phase 3: Issue Detection 🔍

### API Health Checks
- [ ] Test `/api/status` endpoint response time
- [ ] Check for hanging requests
- [ ] Monitor WebSocket connections
- [ ] Check for CORS errors
- [ ] Test concurrent API calls
- [ ] Document timeout issues

### Frontend Issues
- [ ] Check for console errors
- [ ] Verify WebSocket reconnection
- [ ] Test loading states
- [ ] Check for memory leaks
- [ ] Test error boundaries
- [ ] Document UI freezes

## Phase 4: Backend Testing 🔧

### Unit Tests
- [ ] Create `LogBuffer` thread safety test
- [ ] Create log serialization test
- [ ] Create API response time test
- [ ] Create concurrent access test
- [ ] Create memory limit test
- [ ] Run all unit tests

### Integration Tests
- [ ] Test backup start → log generation
- [ ] Test directory processing → log updates
- [ ] Test error scenarios → error logs
- [ ] Test WebSocket → log delivery
- [ ] Test database → log persistence

## Phase 5: Fix Implementation 🛠️

### Backend Fixes
- [ ] **Fix 1: Replace `std::sync::RwLock` with `tokio::sync::RwLock`**
  - [ ] Update LogBuffer implementation
  - [ ] Run thread safety tests
  - [ ] Verify no deadlocks
- [ ] **Fix 2: Implement log pagination**
  - [ ] Add limit parameter to log retrieval
  - [ ] Implement cursor-based pagination
  - [ ] Test with 10,000+ logs
- [ ] **Fix 3: Add proper logging lifecycle**
  - [ ] Log on backup start
  - [ ] Log on directory start
  - [ ] Log on directory completion
  - [ ] Log on errors

### Frontend Fixes
- [ ] **Fix 4: Update API error handling**
  - [ ] Add timeout to fetch calls
  - [ ] Implement retry logic
  - [ ] Show user-friendly error messages
- [ ] **Fix 5: Optimize log rendering**
  - [ ] Implement virtual scrolling for large logs
  - [ ] Add loading states
  - [ ] Debounce updates

## Phase 6: E2E Testing 🎭

### Complete Workflow Test
- [ ] Start with clean state
- [ ] Start backup process
- [ ] Verify logs appear
- [ ] Check progress updates
- [ ] Test pause functionality
- [ ] Test resume functionality
- [ ] Test stop functionality
- [ ] Verify final state

### Screenshot Verification
- [ ] Before backup - all tabs
- [ ] During backup - all tabs
- [ ] After backup - all tabs
- [ ] Error states - all tabs
- [ ] Compare with baseline screenshots

## Phase 7: Performance Testing 📊

### Load Tests
- [ ] Test with 10 directories
- [ ] Test with 100 directories
- [ ] Test with 1,000 directories
- [ ] Test with 10,000 log entries
- [ ] Test with 10 concurrent users
- [ ] Monitor memory usage

### Performance Metrics
- [ ] API response < 100ms ✓/✗
- [ ] WebSocket latency < 50ms ✓/✗
- [ ] UI render < 16ms ✓/✗
- [ ] Memory stable over 1 hour ✓/✗
- [ ] No memory leaks detected ✓/✗

## Phase 8: Verification ✔️

### Final Checks
- [ ] All unit tests pass
- [ ] All integration tests pass
- [ ] All E2E tests pass
- [ ] No console errors
- [ ] Screenshots match expected
- [ ] Performance within limits

### Documentation
- [ ] Update README with test instructions
- [ ] Document all found issues
- [ ] Create fix verification report
- [ ] Update API documentation
- [ ] Create troubleshooting guide

## Phase 9: Sign-off 🎯

### Completion Criteria
- [ ] Logs display in UI
- [ ] No hanging API calls
- [ ] WebSocket stays connected
- [ ] All tests passing
- [ ] Performance acceptable
- [ ] Documentation complete

---

## Test Code Templates

### Playwright Configuration
```typescript
// playwright.config.ts
import { defineConfig } from '@playwright/test';

export default defineConfig({
  testDir: './e2e',
  timeout: 30000,
  use: {
    baseURL: 'http://localhost:3000',
    screenshot: 'on',
    video: 'retain-on-failure',
    trace: 'retain-on-failure',
  },
  projects: [
    {
      name: 'chromium',
      use: { ...devices['Desktop Chrome'] },
    },
  ],
});
```

### E2E Test Examples

#### Backup Tab Inventory
```typescript
// e2e/inventory-ui.spec.ts
test('Inventory Backup Tab', async ({ page }) => {
  await page.goto('/');
  await page.screenshot({ path: 'screenshots/backup-tab-initial.png', fullPage: true });
  
  // Document all elements
  const elements = {
    header: await page.locator('[data-testid="header"]').count(),
    directoryList: await page.locator('[data-testid="directory-item"]').count(),
    controlButtons: await page.locator('button').count(),
    progressBars: await page.locator('[role="progressbar"]').count(),
    stats: await page.locator('[data-testid="stat-card"]').count(),
  };
  
  // Log findings
  console.log('Backup Tab Elements:', elements);
  
  // Test each control
  await testButton(page, 'Start', 'screenshots/backup-after-start.png');
  await testButton(page, 'Pause', 'screenshots/backup-after-pause.png');
  await testButton(page, 'Stop', 'screenshots/backup-after-stop.png');
});
```

#### Issue Detection
```typescript
test.describe('Issue Detection', () => {
  test('Detect hanging API calls', async ({ page }) => {
    let apiHanging = false;
    
    page.on('response', response => {
      if (response.url().includes('/api/status') && response.status() === 0) {
        apiHanging = true;
      }
    });
    
    await page.goto('/');
    await page.waitForTimeout(5000);
    
    expect(apiHanging).toBe(false);
  });
  
  test('Detect WebSocket failures', async ({ page }) => {
    const wsErrors = [];
    
    page.on('console', msg => {
      if (msg.text().includes('WebSocket') && msg.type() === 'error') {
        wsErrors.push(msg.text());
      }
    });
    
    await page.goto('/');
    await page.waitForTimeout(3000);
    
    expect(wsErrors).toHaveLength(0);
  });
});
```

### Backend Test Examples

#### API Performance Test
```rust
// integration/api-tests.rs
#[tokio::test]
async fn test_status_endpoint_performance() {
    let start = Instant::now();
    let response = client.get("/api/status").send().await?;
    let duration = start.elapsed();
    
    assert!(duration.as_millis() < 100);
    assert_eq!(response.status(), 200);
}

#[tokio::test]
async fn test_concurrent_api_calls() {
    let mut handles = vec![];
    
    for _ in 0..10 {
        handles.push(tokio::spawn(async {
            client.get("/api/status").send().await
        }));
    }
    
    for handle in handles {
        let result = handle.await?;
        assert!(result.is_ok());
    }
}
```

---

## Quick Start Commands

```bash
# Run all tests
npm run test:all

# Run E2E tests with UI
npx playwright test --ui

# Run specific test
npx playwright test inventory-ui.spec.ts

# Generate report
npx playwright show-report

# Run backend tests
cd backup-rust && cargo test

# Start test environment
./scripts/test-env.sh
```

## Progress Tracking

- **Total Tasks**: 120
- **Completed**: 0
- **In Progress**: 0
- **Blocked**: 0

**Last Updated**: 2025-06-29
**Next Review**: 2025-07-03

## Execution Timeline

### Week 1: Setup & Discovery
- **Days 1-2**: Set up testing framework
- **Days 3-4**: Run inventory and document issues
- **Day 5**: Prioritize fixes

### Week 2: Implementation
- **Days 1-2**: Implement backend fixes
- **Days 3-4**: Implement frontend fixes
- **Day 5**: Integration testing

### Week 3: Verification & Polish
- **Days 1-2**: Run full test suite
- **Days 3-4**: Performance testing
- **Day 5**: Documentation

---

## Notes Section

### Known Issues
- [ ] Document issue: _____________________
- [ ] Document issue: _____________________
- [ ] Document issue: _____________________

### Blockers
- [ ] Blocker: _____________________
- [ ] Blocker: _____________________

### Questions
- [ ] Question: _____________________
- [ ] Question: _____________________