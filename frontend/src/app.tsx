import React from 'react'
import Badge from 'react-bootstrap/Badge'
import Card from 'react-bootstrap/Card'
import Container from 'react-bootstrap/Container'
import Row from 'react-bootstrap/Row'
import Col from 'react-bootstrap/Col'

import * as calibrator from './calibrator'

function CO2Measure(props: {reading: number}) {
    return (
        <>
            <Badge variant="dark">CO<sub>2</sub></Badge>
            <span className="co2_measurement">
                {props.reading}
            </span>
            <span className="co2_units">ppm</span>
        </>
    );
}

function MetricsURL(props: {url: string}) {
    return (
        <>
            <span className="metrics_url_label">
                Local Metrics URL:
            </span>
            <span className="metrics_url">
                {props.url}
            </span>
        </>
    );
} 

function MeasurementBox(props: {reading: number, url: string}) {
    return (
        <Card role="region" aria-label="co2 measurement box">
            <Card.Body>
                <div className="co2_box">
                    <CO2Measure reading={props.reading} />
                </div>
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
                        <h1>
                            Manage Sensor
                        </h1>
                    </Col>
                </Row>
                <Row className="justify-content-center">
                    <Col md={width}>
                        <MeasurementBox reading={88} url={'http://some.url/metrics'} />
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

export { App };