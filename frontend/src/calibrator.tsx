import React, {FormEvent, useEffect, useState} from 'react';
import Card from 'react-bootstrap/Card';
import Button from 'react-bootstrap/Button';
import Accordion from 'react-bootstrap/Accordion';
import Spinner from 'react-bootstrap/Spinner';
import Collapse from 'react-bootstrap/Collapse';
import Form from 'react-bootstrap/Form';
import InputGroup from 'react-bootstrap/InputGroup';
import FormControl from 'react-bootstrap/FormControl';
import axios from 'axios';
import * as xstate from 'xstate';
import {useMachine} from '@xstate/react';

function calibrationFinished(): [Promise<undefined>, () => void] {
  let resolver: (v?: undefined) => void;
  let rejector: (r?: undefined) => void;
  let p: Promise<undefined> = new Promise((resolve, reject) => {
    resolver = resolve;
    rejector = reject;
  });

  const cancelSource = axios.CancelToken.source();
  const intervalID = setInterval(() => {
    axios
      .get('/isready', {cancelToken: cancelSource.token})
      .then(response => {
        if (response.data) {
          resolver();
        }
      })
      .catch(() => rejector());
  }, 500);

  const cancel = () => {
    cancelSource.cancel('calibration poll cancelled');
    clearInterval(intervalID);
  };

  // Make sure that we clear the interval once the promise is resolved.
  p = p.finally(() => clearInterval(intervalID));
  return [p, cancel];
}

function CalibrationGoOutside(props: {onStart: () => void}) {
  // Special handler that cancels the default form submit (reloading the page)
  // when the user types <Enter> in the form.
  const handleSubmit = (e: FormEvent) => {
    e.preventDefault();
    props.onStart();
  };
  return (
    <>
      <Card.Title>Go Outside</Card.Title>
      <Card.Text>
        The outdoor air has a fairly consistent concentration of CO<sub>2</sub>.
        This well-known concentration will be used as a reference to calibrate
        the sensor. Click next once the device is outdoors.
      </Card.Text>
      <Form onSubmit={handleSubmit} inline>
        <InputGroup className="mb-3 mr-3">
          <FormControl
            id="elevation-value"
            aria-label="Elevation"
            aria-describedby="elevation-units"
            placeholder="Elevation"
          />
          <InputGroup.Append>
            <InputGroup.Text id="elevation-units">ft</InputGroup.Text>
          </InputGroup.Append>
        </InputGroup>
        <Button className="mb-3" variant="primary" onClick={props.onStart}>
          Set and Start
        </Button>
      </Form>
    </>
  );
}

function Calibrating(props: {successful: boolean}) {
  let button = (
    <Button variant="primary" disabled>
      <span className="status-icon">
        <Spinner as="span" animation="border" size="sm" />
      </span>
      Calibrating...
    </Button>
  );
  if (props.successful) {
    button = (
      <Button variant="success" disabled>
        <span className="status-icon">
          <i className="bi bi-check-circle-fill" />
        </span>
        Calibration Successful
      </Button>
    );
  }
  return (
    <>
      <Card.Title>Wait for Calibration</Card.Title>
      <Card.Text>
        The sensor has started the calibration process. It takes several
        measurements for the sensor to calibrate itself, so this step may take
        up to a minute or two.
      </Card.Text>
      {button}
    </>
  );
}

enum OperationButtonOp {
  Calibrate,
  Cancel,
}

type OperationButtonProps = {
  operation: OperationButtonOp;
  disabled?: boolean;
  onCalibrate?: () => void;
  onCancel?: () => void;
};

function OperationButton(props: OperationButtonProps) {
  let variant = 'danger';
  let onClick = props.onCancel;
  let text = 'Cancel';

  if (props.operation === OperationButtonOp.Calibrate) {
    variant = 'primary';
    onClick = props.onCalibrate;
    text = 'Calibrate';
  }

  let disabled = false;
  if (props.disabled !== undefined) {
    disabled = props.disabled;
  }

  return (
    <Button variant={variant} onClick={onClick} disabled={disabled}>
      {text}
    </Button>
  );
}

// State machine representing the calibration workflow.
const calibrationMachine = xstate.Machine({
  id: 'calibration',
  initial: 'closed',
  states: {
    closed: {
      on: {
        OPEN: 'go_outside',
      },
    },
    go_outside: {
      on: {
        START: 'calibration_in_progress',
        CLOSE: 'closed',
      },
    },
    calibration_in_progress: {
      on: {
        DONE: 'calibration_successful',
      },
    },
    calibration_successful: {
      on: {
        CLOSE: 'closed',
      },
    },
  },
});

type WizardProps = {
  // The amount of time to wait before closing the calibration box after
  // calibration completes.
  closeAfterCalibrationMs?: number;
};

function Wizard(props: WizardProps) {
  const onUnmountOnlyFilter: [] = [];
  const [state, send] = useMachine(calibrationMachine);
  const [isOpen, setOpen] = useState(false);

  // General unmount to cancel any pending requests.
  const signal = axios.CancelToken.source();
  useEffect(() => {
    return () => {
      signal.cancel('component unmounting');
    };
  }, onUnmountOnlyFilter);

  // Once we are in the pending state, start polling for calibration to be
  // complete.
  useEffect(() => {
    if (state.value !== 'calibration_in_progress') {
      return undefined;
    }

    // Fire of a background task to wait for the calibration to finish,
    // cancel it on unmount/state change.
    const [ready, cancel] = calibrationFinished();
    (async () => {
      await ready;
      send('DONE');
    })();

    return () => {
      cancel;
    };
  }, [state]);

  // Once we are in the `calibration_successful` state, close the component
  // after the user has had a moment to read the success message.
  useEffect(() => {
    if (state.value !== 'calibration_successful') {
      return undefined;
    }

    let closeAfterCalibrationMs = 1500;
    if (props.closeAfterCalibrationMs !== undefined) {
      closeAfterCalibrationMs = props.closeAfterCalibrationMs;
    }
    const closer = setTimeout(() => {
      setOpen(false);
    }, closeAfterCalibrationMs);
    return () => {
      clearTimeout(closer);
    };
  }, [state]);

  const open = () => {
    send('OPEN');
    setOpen(true);
  };

  const cancel = () => {
    send('CLOSE');
    setOpen(false);
  };

  const start = async () => {
    // TODO: Correctly handle a failed calibration message.
    await axios.put('/calibrate', {}, {cancelToken: signal.token});
    send('START');
  };

  const reset = () => {
    send('CLOSE');
  };

  const calibrationPending = ['closed', 'go_outside'].some(state.matches);

  let body: JSX.Element | null = null;

  if (calibrationPending) {
    body = <CalibrationGoOutside onStart={start} />;
  } else {
    body = (
      <Calibrating successful={state.value === 'calibration_successful'} />
    );
  }

  const headerButtonVariant =
    state.value === 'closed'
      ? OperationButtonOp.Calibrate
      : OperationButtonOp.Cancel;

  return (
    <Accordion>
      <Card>
        <Card.Header>
          <OperationButton
            operation={headerButtonVariant}
            onCalibrate={open}
            onCancel={cancel}
            disabled={!calibrationPending}
          />
        </Card.Header>
        <Collapse in={isOpen} onExited={reset}>
          {/* This div is needed to ensure the collapse/expand
           * animation is smooth. Docs say elements with
           * margin/padding can interfere with the animation */}
          <div>
            <Card.Body role="region" aria-label="Calibration Wizard">
              {body}
            </Card.Body>
          </div>
        </Collapse>
      </Card>
    </Accordion>
  );
}

export {Wizard};
