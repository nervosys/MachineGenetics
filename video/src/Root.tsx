import { Composition } from "remotion";
import { DigitalRain } from "./DigitalRain";

export const RemotionRoot: React.FC = () => {
  return (
    <>
      <Composition
        id="AgenticRain"
        component={DigitalRain}
        durationInFrames={600}   // 20 seconds @ 30fps
        fps={30}
        width={1920}
        height={1080}
      />
    </>
  );
};
