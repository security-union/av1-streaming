import React, { useState, useCallback, useEffect, Dispatch, SetStateAction } from 'react';
import useWebSocket, { ReadyState } from 'react-use-websocket';
import Webcam from 'react-webcam';
import {toByteArray, fromByteArray} from 'base64-js';

const webSocketURL = 'ws://localhost:8080';
let codec_string = "av01.0.04M.08";

export const WebSocketDemo = () => {
  //Public API that will echo messages sent to it back to the client
  const [socketUrl, setSocketUrl] = useState(webSocketURL);
  const [messageHistory, setMessageHistory]: [any[], Dispatch<SetStateAction<any[]>>] = useState([{ data: "sdfsdf"}]);
  const canvasRef = React.useRef(null)
  const webcamRef = React.useRef(null);
  const [videoDecoder, setVideoDecoder] = useState(null);

  const {
    sendMessage,
    lastMessage,
    readyState,
  } = useWebSocket(socketUrl);

  useEffect(() => {
    try {
      const payload = JSON.parse(lastMessage?.data); 
      const data = toByteArray(payload.data);
      const chunk = new EncodedVideoChunk({
        timestamp: payload.timestamp,
        type: payload.type,
        duration: payload.duration,
        data,
      });
      if (payload.type === "key") {
        console.log("got key message");
      }
      // @ts-ignore
      videoDecoder.decode(chunk);
      
    }catch (e: any) {
      console.error("error ", e);
    }
    
    if (videoDecoder === null) {
      // @ts-ignore
      setVideoDecoder(prev => {
        const newEncoder =  new VideoDecoder({
          output: (frame) => {
            console.log("decoded frame");
            const canvas = canvasRef.current;
            // @ts-ignore
            const ctx = canvas.getContext("2d");
            ctx.drawImage(frame, 0, 0);
            frame.close();
          },
          error: (error) => {
            console.error("error", error);
          }
        });
        newEncoder.configure({
          codec: codec_string
        });
        console.log("configured video decoder");
        return newEncoder;
      });
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
    async function captureAndEncode(processChunk: (arg0: EncodedVideoChunk) => void) {
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
          const insert_keyframe = (frame_counter % 50) == 0;
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
    captureAndEncode((chunk: EncodedVideoChunk) => {
      const chunkData = new Uint8Array(chunk.byteLength);
      chunk.copyTo(chunkData);
      const encoded = fromByteArray(chunkData);
      const payload = {
        data: encoded,
        type: chunk.type,
        timestamp: chunk.timestamp,
        duration: chunk.duration
      }
      sendMessage(JSON.stringify(payload), false);
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
      <canvas ref={canvasRef} width={640} height={480}/>
    </div>
  );
};