import React from 'react'
import Badge from 'react-bootstrap/Badge'
import axios from 'axios';
import { CancelTokenSource as AxiosCancelTokenSource } from 'axios';

const READING_POLLING_INTERVAL_MS = 1000;

class Reading extends React.Component<{}, {last_reading?: number}> {
    state = {last_reading: undefined};
    poller?: any = undefined;
    signal?: AxiosCancelTokenSource = undefined;

    componentDidMount() {
        this.signal = axios.CancelToken.source();
        const token = this.signal.token;
        this.poller = setInterval(async () => {
            try {
                const resp = await axios.get('/co2', {cancelToken: token});
                const reading = resp.data;
                this.setState({
                    last_reading: reading,
                });
            } catch {}
        }, READING_POLLING_INTERVAL_MS);
    }

    componentWillUnmount() {
        this.signal?.cancel();
        clearInterval(this.poller);
        this.signal = undefined;
        this.poller = undefined;
    }

    render() {
        let concentration = (<span>loading...</span>);
        if (this.state.last_reading != undefined) {
            concentration = (
                <>
                    <span className="co2_measurement">
                        {this.state.last_reading}
                    </span>
                    <span className="co2_units">ppm</span>
                </>
            );
        }
        return (
            <div className="co2_box">
                <Badge variant="dark">CO<sub>2</sub></Badge>
                {concentration}
            </div>
        );
    }
}

export { Reading };
