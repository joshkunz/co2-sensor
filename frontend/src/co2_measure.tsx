import React, {useEffect, useState} from 'react';
import Badge from 'react-bootstrap/Badge';
import axios from 'axios';

const READING_POLLING_INTERVAL_MS = 1000;

function useCO2Poller(onReading: (reading: number) => void) {
  const onlyOnUnmountFilter: [] = [];
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

  return (
    <div className="co2-box">
      <Badge variant="dark">
        CO<sub>2</sub>
      </Badge>
      <span className="co2-measurement">
        {last_reading === undefined ? 'loading...' : last_reading}
      </span>
      {last_reading !== undefined && <span className="co2-units">ppm</span>}
    </div>
  );
}

export {Reading};
