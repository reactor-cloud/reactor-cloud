import {
  type RequestContext,
  type Result,
  get,
  post,
  del,
  request,
} from '@reactor/shared';

export interface Deployment {
  id: string;
  site_name: string;
  version: string;
  status: 'pending' | 'building' | 'ready' | 'failed';
  url?: string;
  created_at: string;
  completed_at?: string;
}

export interface Domain {
  id: string;
  site_name: string;
  domain: string;
  verified: boolean;
  created_at: string;
}

export interface Site {
  id: string;
  name: string;
  framework?: string;
  created_at: string;
  updated_at: string;
}

export class SitesClient {
  constructor(private ctx: RequestContext) {}

  /**
   * Deploy a site bundle.
   */
  async deploy(
    siteName: string,
    bundle: Blob | ArrayBuffer,
    options?: { framework?: string }
  ): Promise<Result<Deployment>> {
    const formData = new FormData();
    formData.append('bundle', bundle instanceof Blob ? bundle : new Blob([bundle]));
    if (options?.framework) formData.append('framework', options.framework);

    return request(
      this.ctx,
      `/sites/v1/${encodeURIComponent(siteName)}/deploy`,
      { method: 'POST', body: formData }
    );
  }

  /** Domain management */
  get domains() {
    return {
      add: async (siteName: string, domain: string): Promise<Result<Domain>> =>
        post(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/domains`, { domain }),

      remove: async (siteName: string, domain: string): Promise<Result<void>> =>
        del(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/domains/${encodeURIComponent(domain)}`),

      list: async (siteName: string): Promise<Result<Domain[]>> =>
        get(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/domains`),
    };
  }

  /** Deployment management */
  get deployments() {
    return {
      list: async (siteName: string, options?: { limit?: number }): Promise<Result<Deployment[]>> => {
        const params = options?.limit ? `?limit=${options.limit}` : '';
        return get(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/deployments${params}`);
      },

      get: async (siteName: string, deploymentId: string): Promise<Result<Deployment>> =>
        get(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/deployments/${encodeURIComponent(deploymentId)}`),

      rollback: async (siteName: string, deploymentId: string): Promise<Result<Deployment>> =>
        post(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/rollback`, { deployment_id: deploymentId }),

      promote: async (siteName: string, deploymentId: string): Promise<Result<Deployment>> =>
        post(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}/promote`, { deployment_id: deploymentId }),
    };
  }

  /** List all sites */
  async list(): Promise<Result<Site[]>> {
    return get(this.ctx, '/sites/v1');
  }

  /** Get site info */
  async get(siteName: string): Promise<Result<Site>> {
    return get(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}`);
  }

  /** Delete a site */
  async delete(siteName: string): Promise<Result<void>> {
    return del(this.ctx, `/sites/v1/${encodeURIComponent(siteName)}`);
  }
}

export function createSitesClient(ctx: RequestContext): SitesClient {
  return new SitesClient(ctx);
}
