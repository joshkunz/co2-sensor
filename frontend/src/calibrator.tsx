import React, {FormEvent} from 'react';
import Card from 'react-bootstrap/Card';
import Button from 'react-bootstrap/Button';
import Accordion from 'react-bootstrap/Accordion';
import Spinner from 'react-bootstrap/Spinner';
import Collapse from 'react-bootstrap/Collapse';
import Form from 'react-bootstrap/Form';
import InputGroup from 'react-bootstrap/InputGroup';
import FormControl from 'react-bootstrap/FormControl';
import axios from 'axios';
import {CancelTokenSource as AxiosCancelTokenSource} from 'axios';

function calibrationFinished(): [Promise<undefined>, () => void] {
  let resolver: (v?: any) => any;
  let rejector: (r?: any) => any;
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
  onCalibrate?: () => any;
  onCancel?: () => any;
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

type WizardState = {
  step: WizardStep;
  open: boolean;
};

type WizardProps = {
  closeAfterCalibrationMs?: number;
};

enum WizardStep {
  BeforeCalibrate,
  GoOutside,
  CalibrationInProgress,
  CalibrationSuccessful,
}

class Wizard extends React.Component<WizardProps, WizardState> {
  state: WizardState;
  signal: AxiosCancelTokenSource;
  calibrationCancel?: () => void;
  closeTimer?: any = undefined;

  constructor(props: WizardProps) {
    super(props);

    this.state = {
      step: WizardStep.BeforeCalibrate,
      open: false,
    };

    this.signal = axios.CancelToken.source();

    this.calibrate = this.calibrate.bind(this);
    this.cancel = this.cancel.bind(this);
    this.start = this.start.bind(this);
    this.reset = this.reset.bind(this);
  }

  componentDidMount() {
    this.signal = axios.CancelToken.source();
  }

  componentWillUnmount() {
    this.signal.cancel('component unmounting');
    if (this.calibrationCancel !== undefined) {
      this.calibrationCancel();
      this.calibrationCancel = undefined;
    }
    if (this.closeTimer !== undefined) {
      clearTimeout(this.closeTimer);
    }
  }

  calibrate() {
    this.setState({
      step: WizardStep.GoOutside,
      open: true,
    });
  }

  cancel() {
    this.setState({
      step: WizardStep.BeforeCalibrate,
      open: false,
    });
  }

  reset() {
    this.setState({step: WizardStep.BeforeCalibrate});
  }

  async start() {
    // TODO: Correctly handle a failed calibration message.
    await axios.put(
      '/calibrate',
      {},
      {
        cancelToken: this.signal.token,
      }
    );
    this.setState({step: WizardStep.CalibrationInProgress});
    const [ready, cancel] = calibrationFinished();
    this.calibrationCancel = cancel;
    await ready;
    this.setState({step: WizardStep.CalibrationSuccessful});

    let closeAfterCalibrationMs = 1500;
    if (this.props.closeAfterCalibrationMs !== undefined) {
      closeAfterCalibrationMs = this.props.closeAfterCalibrationMs;
    }
    this.closeTimer = setTimeout(() => {
      this.closeTimer = undefined;
      this.setState({open: false});
    }, closeAfterCalibrationMs);
  }

  stepIs(want: WizardStep): boolean {
    return this.state.step === want;
  }

  render() {
    const calibrationPending =
      this.stepIs(WizardStep.BeforeCalibrate) ||
      this.stepIs(WizardStep.GoOutside);

    let body: JSX.Element | null = null;

    if (calibrationPending) {
      body = <CalibrationGoOutside onStart={this.start} />;
    } else {
      body = (
        <Calibrating
          successful={this.state.step === WizardStep.CalibrationSuccessful}
        />
      );
    }

    const headerButtonVariant = this.stepIs(WizardStep.BeforeCalibrate)
      ? OperationButtonOp.Calibrate
      : OperationButtonOp.Cancel;

    return (
      <Accordion>
        <Card>
          <Card.Header>
            <OperationButton
              operation={headerButtonVariant}
              onCalibrate={this.calibrate}
              onCancel={this.cancel}
              disabled={!calibrationPending}
            />
          </Card.Header>
          <Collapse in={this.state.open} onExited={this.reset}>
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
}

export {Wizard, WizardStep};
