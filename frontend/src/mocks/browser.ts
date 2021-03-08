import {setupWorker, rest} from 'msw';
import {Chance} from 'chance';

class State {
  calibration_pending = false;
  co2_ppm = 88;

  constructor() {
    const chance = new Chance();
    setInterval(() => {
      this.co2_ppm = chance.natural({min: 100, max: 2000});
    }, 15000);
  }

  calibrate() {
    this.calibration_pending = true;
    setTimeout(() => {
      this.calibration_pending = false;
    }, 1500);
  }
}

const globalState = new State();

export const worker = setupWorker(
  rest.put('/calibrate', (_, res, ctx) => {
    globalState.calibrate();
    return res(ctx.status(200));
  }),
  rest.get('/isready', (_, res, ctx) => {
    return res(ctx.json(!globalState.calibration_pending));
  }),
  rest.get('/co2', (_, res, ctx) => {
    return res(ctx.json(globalState.co2_ppm));
  })
);
