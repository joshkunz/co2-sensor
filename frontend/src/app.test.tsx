import React from 'react';
import {render, screen} from '@testing-library/react';
import '@testing-library/jest-dom';

import * as app from './app';

test('page contains co2 measurement box', () => {
  render(<app.App />);
  expect(
    screen.queryByRole('region', {
      name: 'co2 measurement box',
    })
  ).not.toBeNull();
});

test('page contains calibration button', () => {
  render(<app.App />);
  expect(screen.getByRole('button', {name: 'Calibrate'})).toBeVisible();
});

test('page contains metrics URL', () => {
  render(<app.App />);
  expect(screen.getByText(/Local Metrics URL/)).toBeVisible();
});

test('page has manage sensor title', () => {
  render(<app.App />);
  expect(screen.getByRole('heading', {name: 'Manage Sensor'})).toBeVisible();
});
