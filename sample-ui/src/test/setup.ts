import '@testing-library/jest-dom';
import { cleanup } from '@solidjs/testing-library';
import { afterEach } from 'vitest';

// Cleanup after each test
afterEach(() => {
  cleanup();
});

// Mock window.crypto.randomUUID for tests
Object.defineProperty(global, 'crypto', {
  value: {
    randomUUID: () => '00000000-0000-0000-0000-000000000000',
  },
});
