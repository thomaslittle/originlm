import {useEffect, useId, useRef} from 'react';
import geometry from '../../../../brand/logo-geometry.json';
import {trackBrandEye} from './brand-eye-motion';

/** Supplied contours: only the large O's negative-space opening moves. */
export function BrandLogo() {
  const id = `originlm-eye-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`;
  const svg = useRef<SVGSVGElement>(null), counter = useRef<SVGPathElement>(null);
  useEffect(() => {
    if (svg.current && counter.current) return trackBrandEye(svg.current, counter.current);
  }, []);
  return <svg ref={svg} width={906} height={208} viewBox="0 0 906 208" role="img" aria-label="OriginLM">
    <defs><mask id={id} x="0" y="0" width="205" height="208" maskUnits="userSpaceOnUse" style={{maskType:'luminance'}}>
      <rect width="205" height="208" fill="white" />
      <path ref={counter} data-logo-eye="" d={geometry.mark.paths[1].d} fill="black" transform="translate(0 0)" />
    </mask></defs>
    <path data-logo-fixed="outer" d={geometry.mark.paths[0].d} fill="#EBA31A" mask={`url(#${id})`} />
    <path data-logo-fixed="wordmark" d={geometry.wordmark.paths.map(path=>path.d).join(' ')} fill="white" fillRule="evenodd" transform="translate(244 38)" />
  </svg>;
}
