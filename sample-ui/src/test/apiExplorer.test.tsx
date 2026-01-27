import { describe, it, expect, vi, beforeEach } from 'vitest';
import { render, screen, waitFor } from '@solidjs/testing-library';
import userEvent from '@testing-library/user-event';
import App from '../App';

// Mock fetch globally
global.fetch = vi.fn();

describe('API Explorer', () => {
  beforeEach(() => {
    vi.clearAllMocks();
    
    // Mock OpenAPI spec fetch
    (global.fetch as any).mockImplementation((url: string) => {
      if (url.includes('/openapi.yaml')) {
        return Promise.resolve({
          ok: true,
          text: async () => `
paths:
  /pets:
    get:
      operationId: list_pets
  /users:
    get:
      operationId: list_users
  /items/{id}:
    post:
      operationId: post_item
`,
        });
      }
      if (url.includes('/pets') || url.includes('/users')) {
        return Promise.resolve({
          ok: true,
          json: async () => [],
        });
      }
      return Promise.resolve({
        ok: false,
        status: 404,
      });
    });
  });

  it('should render API Explorer button', () => {
    render(() => <App />);
    const button = screen.getByText(/📖 API Explorer/i);
    expect(button).toBeInTheDocument();
  });

  it('should open API Explorer modal when button is clicked', async () => {
    const user = userEvent.setup();
    render(() => <App />);
    
    const button = screen.getByText(/📖 API Explorer/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/API Explorer/i)).toBeInTheDocument();
    });
  });

  it('should display endpoints from OpenAPI spec', async () => {
    const user = userEvent.setup();
    render(() => <App />);
    
    const button = screen.getByText(/📖 API Explorer/i);
    await user.click(button);

    await waitFor(() => {
      expect(screen.getByText(/GET/i)).toBeInTheDocument();
      expect(screen.getByText(/\/pets/i)).toBeInTheDocument();
    }, { timeout: 3000 });
  });

  it('should test GET endpoint', async () => {
    const user = userEvent.setup();
    const mockResponse = {
      ok: true,
      status: 200,
      json: async () => [{ id: 1, name: 'Test' }],
      headers: new Headers(),
    };

    (global.fetch as any).mockResolvedValueOnce(mockResponse);

    render(() => <App />);
    
    const button = screen.getByText(/📖 API Explorer/i);
    await user.click(button);

    await waitFor(() => {
      const tryButton = screen.getByText(/🚀 Try it/i);
      expect(tryButton).toBeInTheDocument();
    });

    const tryButton = screen.getByText(/🚀 Try it/i);
    await user.click(tryButton);

    await waitFor(() => {
      expect(global.fetch).toHaveBeenCalledWith(
        expect.stringContaining('/pets'),
        expect.objectContaining({
          method: 'GET',
          headers: expect.objectContaining({
            'X-API-Key': 'test123',
          }),
        })
      );
    });
  });
});
