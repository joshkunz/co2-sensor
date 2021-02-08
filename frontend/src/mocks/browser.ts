import { setupWorker, rest } from 'msw'

class State {
    calibration_pending: boolean = false;

    calibrate() {
        this.calibration_pending = true;
        setTimeout(() => {
            this.calibration_pending = false;
        }, 1500);
    }
}

let globalState = new State();

export const worker = setupWorker(
    rest.put('/calibrate', (_, res, ctx) => {
        globalState.calibrate();
        return res(ctx.status(200));
    }),
    rest.get('/isready', (_, res, ctx) => {
        return res(ctx.json(!globalState.calibration_pending));
    })
);