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
    const button = screen.getByRole('button', { name: /💰 Money\/Currency Tester/i });
    expect(button).toBeInTheDocument();
  });

  it('should open money tester modal when button is clicked', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    const button = screen.getByRole('button', { name: /💰 Money\/Currency Tester/i });
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByRole('heading', { name: /Money\/Currency Type Tester/i })).toBeInTheDocument();
    });
  });

  it('should display example request with money format', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    await waitFor(() => {
      expect(screen.getByText(/Example Request \(format: money in OpenAPI\):/i)).toBeInTheDocument();
      expect(screen.getByText(/Payment Test \(format: money\)/i)).toBeInTheDocument();
      expect(screen.getAllByText(/format: money/i).length).toBeGreaterThan(0);
    });
  });

  it('should demonstrate payment format for 3.14 USD', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    const demoButton = screen.getByRole('button', { name: /Demonstrate Money Format \(3\.14 USD\)/i });
    await user.click(demoButton);

    await waitFor(() => {
      expect(screen.getByText(/Test Results/i)).toBeInTheDocument();
      expect(screen.getByText(/Money Format Demonstration/i)).toBeInTheDocument();
      expect(screen.getAllByText(/314 cents/i).length).toBeGreaterThan(0);
    });
  });

  it('should confirm the example request includes the 3.14 USD money value', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    await waitFor(() => {
      expect(screen.getByText(/Test money types with amount:/i)).toBeInTheDocument();
      expect(screen.getAllByText(/3\.14/i).length).toBeGreaterThan(0);
    });
  });

  it('should test different currencies (USD, EUR, GBP, JPY)', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    await waitFor(() => {
      expect(screen.getByText(/Multi-Currency Examples/i)).toBeInTheDocument();
    });

    const currencies = ['USD', 'EUR', 'GBP', 'JPY'];
    for (const currency of currencies) {
      const currencyButton = screen.getByRole('button', { name: new RegExp(`Demonstrate ${currency}`, 'i') });
      await user.click(currencyButton);

      await waitFor(() => {
        expect(screen.getByText(new RegExp(`${currency} Format Demonstration`, 'i'))).toBeInTheDocument();
      });
    }
  });

  it('should show the successful money-format demonstration result', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    await user.click(screen.getByRole('button', { name: /Demonstrate Money Format/i }));

    await waitFor(() => {
      expect(screen.getByText(/Test Results/i)).toBeInTheDocument();
      expect(screen.getByText(/Money Format Demonstration/i)).toBeInTheDocument();
    });
  });

  it('should display test results with request and response data', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));
    await user.click(screen.getByRole('button', { name: /Demonstrate Money Format/i }));

    await waitFor(() => {
      expect(screen.getByText(/Test Results/i)).toBeInTheDocument();
      expect(screen.getByText(/Request Data \(Example\):/i)).toBeInTheDocument();
      expect(screen.getByText(/Response Data:/i)).toBeInTheDocument();
    });
  });

  it('should display info about money types', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));

    await waitFor(() => {
      expect(screen.getByText(/About Money Types/i)).toBeInTheDocument();
      expect(screen.getAllByText(/rusty_money::Money/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/rust_decimal::Decimal/i).length).toBeGreaterThan(0);
      expect(screen.getAllByText(/314 cents/i).length).toBeGreaterThan(0);
    });
  });

  it('should describe the example POST /items endpoint format', async () => {
    const user = userEvent.setup();
    render(() => <App />);

    await user.click(screen.getByRole('button', { name: /💰 Money\/Currency Tester/i }));
    await user.click(screen.getByRole('button', { name: /Demonstrate USD/i }));

    await waitFor(() => {
      expect(screen.getByText(/POST \/items/i)).toBeInTheDocument();
    });
  });
});
