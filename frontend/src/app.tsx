import React from 'react';
import Card from 'react-bootstrap/Card';
import Container from 'react-bootstrap/Container';
import Row from 'react-bootstrap/Row';
import Col from 'react-bootstrap/Col';

import * as calibrator from './calibrator';
import * as co2_measure from './co2_measure';

function MetricsURL(props: {url: string}) {
  return (
    <>
      <span className="metrics_url_label">Local Metrics URL:</span>
      <span className="metrics_url">{props.url}</span>
    </>
  );
}

function MeasurementBox(props: {url: string}) {
  return (
    <Card role="region" aria-label="co2 measurement box">
      <Card.Body>
        <co2_measure.Reading />
        <MetricsURL url={props.url} />
      </Card.Body>
    </Card>
  );
}

class App extends React.Component<{}, {}> {
  constructor(props: {}) {
    super(props);
  }

  render() {
    // In Bootstrap columns.
    const width = 6;

    return (
      <Container fluid>
        <Row className="justify-content-center">
          <Col md={width}>
            <h1>Manage Sensor</h1>
          </Col>
        </Row>
        <Row className="justify-content-center">
          <Col md={width}>
            <MeasurementBox url={'http://some.url/metrics'} />
          </Col>
        </Row>
        <Row className="mt-2 justify-content-center">
          <Col md={width}>
            <calibrator.Wizard />
          </Col>
        </Row>
      </Container>
    );
  }
}

export {App};
