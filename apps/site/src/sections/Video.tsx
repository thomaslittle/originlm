import { Play } from "../components/Icon";
import { PROMO } from "../data/video";

const CHAPTERS = [
  ["00:03", "Know your AI", "Reported allowances, reset times and clearly labeled cost estimates."],
  ["00:10", "Build on it", "The exploded stack, from providers up to your app."],
  ["00:19", "Connect", "An example discovery grant, separate from execution permission."],
  ["00:23", "Install", "A local package and the public discovery API."],
];

export function Video() {
  return (
    <section className="section video" id="video" aria-labelledby="video-title">
      <div className="container">
        <div className="head-split">
          <h2 id="video-title">Two sides of OriginLM. In 35 seconds.</h2>
          <p className="lead">
            A silent product tour: first what you can see, then what you can build.
            The source uses fframes and the OriginLM palette.
          </p>
        </div>

        <div className="video-layout">
          <figure className="video-frame">
            {PROMO.hasVideo ? (
              <video
                controls
                preload="metadata"
                playsInline
                poster={PROMO.poster}
                aria-label="OriginLM product film: know your AI usage, then build on it."
              >
                <source src={PROMO.webm} type="video/webm" />
                <source src={PROMO.mp4} type="video/mp4" />
                Your browser cannot play this video. The poster image and the sections above cover the
                same material.
              </video>
            ) : (
              <div className="video-poster" data-slot="film">
                <img src="/video-poster.svg" alt="" width={1600} height={900} loading="lazy" />
                <div className="video-placeholder" data-keep>
                  <span className="video-play" aria-hidden="true"><Play size={26} /></span>
                  <div>
                    <h3>The 35-second tour</h3>
                    <p>
                      The film is rendered separately from this source checkout. The product tour above
                      covers the same features; see media/video for the reproducible film source.
                    </p>
                  </div>
                </div>
              </div>
            )}
            <figcaption>
              {PROMO.hasVideo ? "1920×1080, H.264 and WebM. 35 seconds, silent." : "Film assets are not included in this build."} Source: media/video.
            </figcaption>
          </figure>

          <ol className="chapters">
            {CHAPTERS.map(([time, title, body]) => (
              <li key={title}>
                <time>{time}</time>
                <h3>{title}</h3>
                <p>{body}</p>
              </li>
            ))}
          </ol>
        </div>
      </div>
    </section>
  );
}
