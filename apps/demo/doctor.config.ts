import type { ReactDoctorConfig } from 'react-doctor/api';

/**
 * React Doctor is a required check on every pull request (see .github/workflows/react-doctor.yml)
 * and gates on warnings, so each exclusion below states why it is not a finding in our code.
 */
export default {
  ignore: {
    files: [
      // shadcn/ui components are vendor code installed and updated by the shadcn CLI
      // (`components.json`, style base-mira). Edits there are limited to documented
      // extensions (semantic Badge/Alert variants) and are reviewed against the CLI output.
      'src/components/ui/**',
      // Plain Node ESM fixture service and mock transport (not React, not shipped to the browser).
      'fixture/**',
    ],
  },
} satisfies ReactDoctorConfig;
