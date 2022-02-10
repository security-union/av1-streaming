import React, { useState, useCallback, useEffect, Dispatch, SetStateAction } from 'react';
import useWebSocket, { ReadyState } from 'react-use-websocket';
import Webcam from 'react-webcam';
const webSocketURL = 'ws://localhost:10000';
let codec_string = "av01.0.04M.08";

export const WebSocketDemo = () => {
  //Public API that will echo messages sent to it back to the client
  const [socketUrl, setSocketUrl] = useState(webSocketURL);
  const [messageHistory, setMessageHistory]: [any[], Dispatch<SetStateAction<any[]>>] = useState([{ data: "sdfsdf"}]);
  const webcamRef = React.useRef(null);

  const {
    sendMessage,
    lastMessage,
    readyState,
  } = useWebSocket(socketUrl);

  useEffect(() => {
    if (typeof lastMessage === 'object') {
      setMessageHistory(prev => prev.concat(lastMessage));
    }
  }, [lastMessage, setMessageHistory]);

  const handleClickChangeSocketUrl = useCallback(() =>
    setSocketUrl(webSocketURL), []);

  const handleClickSendMessage = useCallback(() =>
    sendMessage('Hello'), []);

  const connectionStatus = {
    [ReadyState.CONNECTING]: 'Connecting',
    [ReadyState.OPEN]: 'Open',
    [ReadyState.CLOSING]: 'Closing',
    [ReadyState.CLOSED]: 'Closed',
    [ReadyState.UNINSTANTIATED]: 'Uninstantiated',
  }[readyState];

  const [capturing, setCapturing] = React.useState(false);

  const handleStartCaptureClick = React.useCallback(() => {
    setCapturing(true);
    async function captureAndEncode(processChunk: any) {
      if (webcamRef.current !== null) {
        // @ts-ignore
        const stream = webcamRef.current.stream as MediaStream;
        let frame_counter = 0;
        var track = stream.getTracks()[0];
        var settings = track.getSettings();
        var pending_outputs = 0;
        // @ts-ignore
        var prc = new MediaStreamTrackProcessor(track);
        var frameStream = prc.readable;
        const frameReader= frameStream.getReader();
      
        const init = {
          // @ts-ignore
          output: (chunk) => {
            pending_outputs--;
            processChunk(chunk);
          },
          error: (e: Error) => {
            console.error(e.message);
          }
        };
      
        const config = {
          codec: codec_string,
          width: settings.width!,
          height: settings.height!
        };
      
        let encoder = new VideoEncoder(init);
        encoder.configure(config);
        // @ts-ignore
        frameReader.read().then(function processFrame({done, value}) { 
          if(done||capturing) {
            value.close();
            encoder.close();
            return;
          }
    
        if (!capturing && pending_outputs <= 30) {
          if (++frame_counter % 20 == 0) {
            console.log(frame_counter);
          }
    
          pending_outputs++;
          const insert_keyframe = (frame_counter % 150) == 0;
          encoder.encode(value, { keyFrame: insert_keyframe });
        }
        value.close();
        frameReader.read().then(processFrame);
      });
        
      } else {
        console.error("error!!!");
      }
    }
    // @ts-ignore
    captureAndEncode((chunk) => {
      sendMessage(chunk, false);
    });
    
  }, [webcamRef, setCapturing]);

  const handleStopCaptureClick = React.useCallback(() => {
    setCapturing(false);
  }, [webcamRef, setCapturing]);

  return (
    <div>
      <Webcam audio={false} ref={webcamRef} />
      {capturing ? (
        <button onClick={handleStopCaptureClick}>Stop Capture</button>
      ) : (
        <button onClick={handleStartCaptureClick}>Start Capture</button>
      )}
      <button
        onClick={handleClickChangeSocketUrl}
      >
        Click Me to change Socket Url
      </button>
      <button
        onClick={handleClickSendMessage}
        disabled={readyState !== ReadyState.OPEN}
      >
        Click Me to send 'Hello'
      </button>
      <span>The WebSocket is currently {connectionStatus}</span>
      {lastMessage ? <span>Last message: {lastMessage.data}</span> : null}
      <ul>
        {messageHistory
          .map((message, idx) => <span key={idx}>{message?.data}</span>)}
      </ul>
    </div>
  );
};