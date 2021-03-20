import React, {useEffect, useState} from 'react';
import Badge from 'react-bootstrap/Badge';
import axios from 'axios';

const READING_POLLING_INTERVAL_MS = 1000;

function useCO2Poller(onReading: (reading: number) => void) {
  const onlyOnUnmountFilter: any[] = [];
  useEffect(() => {
    const signal = axios.CancelToken.source();
    const poller = setInterval(async () => {
      try {
        const resp = await axios.get('/co2', {cancelToken: signal.token});
        const reading = resp.data;
        onReading(reading);
      } catch {
        // TODO(jkz): Properly handle actual fetch failures by notifying
        // the user.
      }
    }, READING_POLLING_INTERVAL_MS);
    return () => {
      signal.cancel();
      clearInterval(poller);
    };
  }, onlyOnUnmountFilter);
}

function Reading() {
  const [last_reading, setLastReading] = useState<number | undefined>(
    undefined
  );
  useCO2Poller(r => {
    setLastReading(r);
  });

  let concentration = <span>loading...</span>;
  if (last_reading !== undefined) {
    concentration = (
      <>
        <span className="co2_measurement">{last_reading}</span>
        <span className="co2_units">ppm</span>
      </>
    );
  }
  return (
    <div className="co2_box">
      <Badge variant="dark">
        CO<sub>2</sub>
      </Badge>
      {concentration}
    </div>
  );
}

export {Reading};
