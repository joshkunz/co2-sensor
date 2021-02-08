import React from 'react'
import { act } from 'react-dom/test-utils'
import {render, screen} from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import '@testing-library/jest-dom'
import * as msw from 'msw'
import * as mswNode from 'msw/node'

import { Wizard } from './calibrator'

// Set up our mocked msw server, and clear it after each test runs.
const server = mswNode.setupServer();
beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());

test('wizard contains calibrate button by default', () => {
    render(<Wizard />);
    expect(screen.getByRole('button', {name: 'Calibrate'}))
        .toBeVisible();
});

test('wizard calibrate button opens dialog', () => {
    render(<Wizard />);
    userEvent.click(screen.getByText("Calibrate"));

    // Calibrate button should turn into a red cancel button.
    expect(screen.queryByRole('button', {name: 'Calibrate'})).not.toBeInTheDocument();
    expect(screen.getByRole('button', {name: 'Cancel'}))
        .toHaveClass("btn-danger");

    // Should have a new region displayed that contains our calibration wizard.
    expect(screen.getByRole('region', {name: 'Calibration Wizard'}))
        .toBeVisible();
    
    expect(screen.getByRole('button', {name: 'Start'}))
        .toBeVisible();
    
    // Include a prompt for the user go outside before starting the calibration.
    expect(screen.getByText('Go Outside')).toBeVisible();
});

test('wizard successfull calibration', async () => {
    let calibrationStarted: boolean = false;
    let isReady: boolean = false;
    server.use(
        msw.rest.put('/calibrate', (_, res, ctx) => {
            calibrationStarted = true;
            return res(ctx.status(200));
        }),
        msw.rest.get('/isready', (_, res, ctx) => {
            return res(ctx.json(isReady));
        })
    );

    jest.useFakeTimers();

    render(<Wizard  />);

    // Click "Calibrate" to open the calibration dialog.
    userEvent.click(screen.getByRole('button', {name: 'Calibrate'}));

    // Click "Start" to start the calibration.
    userEvent.click(screen.getByRole('button', {name: 'Start'}));

    // Wait for the calibration flow to be started, and the "Calibrating..."
    // response to appear.
    await screen.findByText('Calibrating...');

    // Make sure the prompt we found is visible.
    expect(screen.getByText('Calibrating...')).toBeVisible();

    // Make sure that our code called the calibration handler to start
    // calibration.
    expect(calibrationStarted).toBe(true);

    // Assert that the previous content is removed.
    expect(screen.queryByText('Go Outside')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', {name: 'Start'})).not.toBeInTheDocument();

    // And our cancel button should be disabled.
    expect(screen.getByRole('button', {name: 'Cancel'})).toBeDisabled();

    // Complete the calibration.
    isReady = true;

    await screen.findByText('Calibration Successful');

    expect(screen.getByRole('button', {name: 'Calibration Successful'}))
        .toBeDisabled();

    act(() => { jest.runAllTimers() });
    
    // Give a longer timeout here to make sure we wait long enough to see
    // the successful calibration disappear.
    await screen.findByRole('button', {name: 'Calibrate'});

    // Our old "calibration successful" message should be gone now.
    expect(screen.queryByRole('button', {name: 'Calibration Successful'}))
        .not.toBeInTheDocument();
})