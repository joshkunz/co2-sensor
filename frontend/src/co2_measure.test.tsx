import React from 'react';
import {render, screen} from '@testing-library/react';
import '@testing-library/jest-dom';
import * as msw from 'msw';
import * as mswNode from 'msw/node';

import {Reading} from './co2_measure';

// Set up our mocked msw server, and clear it after each test runs.
const server = mswNode.setupServer();
beforeAll(() => server.listen());
afterEach(() => {
  jest.runOnlyPendingTimers();
  jest.useRealTimers();
  server.resetHandlers();
});
afterAll(() => server.close());

test('reading displays value fetched from the server', async () => {
  server.use(
    msw.rest.get('/co2', (_, res, ctx) => {
      return res(ctx.json(150));
    })
  );

  jest.useFakeTimers();

  render(<Reading />);

  jest.advanceTimersByTime(2000);

  const found = await screen.findByText('150');
  expect(found).toBeInTheDocument();
  // Assert that we also have the "ppm" in the next element.
  expect(found.nextSibling?.textContent).toBe('ppm');
});
