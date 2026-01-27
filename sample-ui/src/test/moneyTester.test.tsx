import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import App from '../App';

// Mock fetch globally
global.fetch = vi.fn();

describe('Money Tester', () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it('should render money tester button', () => {
    render(() => <App />);
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    expect(button).toBeInTheDocument();
  });

  it('should open money tester modal when button is clicked', async () => {
    const user = userEvent.setup();
    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/Money\/Currency Type Tester/i)).toBeInTheDocument();
    });
  });

  it('should display example request with money format', async () => {
    const user = userEvent.setup();
    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/Example Request/i)).toBeInTheDocument();
      expect(screen.getByText(/3.14/i)).toBeInTheDocument();
      expect(screen.getByText(/format: money/i)).toBeInTheDocument();
    });
  });

  it('should test payment with 3.14 USD', async () => {
    const user = userEvent.setup();
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({ id: 'test-id', name: 'Test Payment Item' }),
    };

    (global.fetch as any).mockResolvedValueOnce(mockResponse);

    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
      expect(testButton).toBeInTheDocument();
    });

    const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
    await user.click(testButton);

    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        expect.stringContaining('/items/'),
        expect.objectContaining({
          method: 'POST',
          headers: expect.objectContaining({
            'Content-Type': 'application/json',
          }),
        })
      );
    });
  });

  it('should test different currencies (USD, EUR, GBP, JPY)', async () => {
    const user = userEvent.setup();
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({ id: 'test-id', name: 'Test Item' }),
    };

    (global.fetch as any).mockResolvedValue(mockResponse);

    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/Multi-Currency Examples/i)).toBeInTheDocument();
    });

    // Test each currency
    const currencies = ['USD', 'EUR', 'GBP', 'JPY'];
    for (const currency of currencies) {
      const currencyButton = screen.getByText(new RegExp(`Test ${currency}`, 'i'));
      expect(currencyButton).toBeInTheDocument();
      
      await user.click(currencyButton);
      
      await waitFor(() => {
        expect(global.fetch).toHaveBeenCalledWith(
          expect.stringContaining('/items/'),
          expect.objectContaining({
            method: 'POST',
            body: expect.stringContaining(`"name":"Test Item (${currency})"`),
          })
        );
      });
    }
  });

  it('should handle API errors gracefully', async () => {
    const user = userEvent.setup();
    const mockErrorResponse = {
      ok: false,
      status: 404,
      statusText: 'Not Found',
      json: async () => ({ error: 'Not Found', method: 'POST', path: '/items/test-id' }),
    };

    (global.fetch as any).mockResolvedValueOnce(mockErrorResponse);

    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
      expect(testButton).toBeInTheDocument();
    });

    const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
    await user.click(testButton);

    await waitFor(() => {
      expect(screen.getByText(/Error:/i)).toBeInTheDocument();
    });
  });

  it('should display test results with request and response data', async () => {
    const user = userEvent.setup();
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({ 
        id: 'test-uuid-123',
        name: 'Test Payment Item'
      }),
    };

    (global.fetch as any).mockResolvedValueOnce(mockResponse);

    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
      expect(testButton).toBeInTheDocument();
    });

    const testButton = screen.getByText(/🚀 Test Payment \(3.14 USD\)/i);
    await user.click(testButton);

    await waitFor(() => {
      expect(screen.getByText(/Test Results/i)).toBeInTheDocument();
      expect(screen.getByText(/Request Data/i)).toBeInTheDocument();
      expect(screen.getByText(/Response Data/i)).toBeInTheDocument();
    });
  });

  it('should display info about money types', async () => {
    const user = userEvent.setup();
    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/About Money Types/i)).toBeInTheDocument();
      expect(screen.getByText(/rusty_money::Money/i)).toBeInTheDocument();
      expect(screen.getByText(/rust_decimal::Decimal/i)).toBeInTheDocument();
      expect(screen.getByText(/314 cents/i)).toBeInTheDocument();
    });
  });

  it('should use correct endpoint format /items/{id} with UUID', async () => {
    const user = userEvent.setup();
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => ({ id: 'test-id', name: 'Test Item' }),
    };

    (global.fetch as any).mockResolvedValueOnce(mockResponse);

    render(() => <App />);
    
    const button = screen.getByText(/💰 Money\/Currency Tester/i);
    await user.click(button);

    await waitFor(() => {
      const testButton = screen.getByText(/Test USD/i);
      expect(testButton).toBeInTheDocument();
    });

    const testButton = screen.getByText(/Test USD/i);
    await user.click(testButton);

    await waitFor(() => {
      const fetchCalls = (global.fetch as any).mock.calls;
      const lastCall = fetchCalls[fetchCalls.length - 1];
      const url = lastCall[0];
      
      // Should match /items/{uuid} pattern
      expect(url).toMatch(/\/items\/[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}/i);
    });
  });
});
