// AfyaMkononi website: Azure Front Door Premium + Web Application Firewall in
// front of the Azure Static Web App.
//
// What this gives the static site (which has no server of its own):
//   - Rate limiting per client IP, with the IP blocked for the rest of the
//     window once it goes over (Front Door's equivalent of a fail2ban jail).
//   - A manual IP jail list for addresses you want blocked outright.
//   - Scanner trap: requests for WordPress/PHP/.env/.git style paths are
//     blocked (nothing like that exists on this site, so only attackers ask).
//   - Only GET/HEAD/OPTIONS allowed.
//   - Microsoft's managed rule sets: OWASP-style attack signatures and the
//     bot manager (known bad bots blocked, good crawlers allowed).
//
// Deploy (resource group must exist):
//   az deployment group create -g <rg> -f deploy/afyamkononi-website/frontdoor.bicep \
//     -p staticWebAppHostname=<name>.azurestaticapps.net
// Then lock the Static Web App to this Front Door (see the website README,
// section "Security", step "Only Front Door may reach the site").

@description('Default hostname of the Static Web App, e.g. polite-sea-0a1b2c3.azurestaticapps.net')
param staticWebAppHostname string

@description('Name prefix for the Front Door resources.')
param name string = 'afyamkononi-web'

@description('Requests allowed per client IP per minute before that IP is blocked for the rest of the minute. Kenyan mobile networks put many people behind one IP (carrier NAT), so keep this generous; one page view is about 30 requests.')
@minValue(100)
param requestsPerIpPerMinute int = 1500

@description('Requests allowed per client IP per 5 minutes; the longer "jail" for sustained scraping or flooding.')
@minValue(500)
param requestsPerIpPer5Minutes int = 4000

@description('IP addresses or CIDR ranges to block outright (manual jail list).')
param blockedIpRanges array = []

@description('Detection logs only (Detection) or block (Prevention). Start in Detection for a few days if unsure, then switch to Prevention.')
@allowed([
  'Detection'
  'Prevention'
])
param wafMode string = 'Prevention'

var wafName = replace('${name}waf', '-', '')

resource waf 'Microsoft.Network/FrontDoorWebApplicationFirewallPolicies@2024-02-01' = {
  name: wafName
  location: 'Global'
  sku: {
    name: 'Premium_AzureFrontDoor'
  }
  properties: {
    policySettings: {
      enabledState: 'Enabled'
      mode: wafMode
      requestBodyCheck: 'Enabled'
      customBlockResponseStatusCode: 403
    }
    customRules: {
      rules: concat(empty(blockedIpRanges) ? [] : [
        {
          name: 'IpJailManual'
          priority: 10
          enabledState: 'Enabled'
          ruleType: 'MatchRule'
          action: 'Block'
          matchConditions: [
            {
              matchVariable: 'SocketAddr'
              operator: 'IPMatch'
              negateCondition: false
              matchValue: blockedIpRanges
            }
          ]
        }
      ], [
        {
          name: 'OnlyReadMethods'
          priority: 20
          enabledState: 'Enabled'
          ruleType: 'MatchRule'
          action: 'Block'
          matchConditions: [
            {
              matchVariable: 'RequestMethod'
              operator: 'Equal'
              negateCondition: true
              matchValue: [
                'GET'
                'HEAD'
                'OPTIONS'
              ]
            }
          ]
        }
        {
          name: 'ScannerTrap'
          priority: 30
          enabledState: 'Enabled'
          ruleType: 'MatchRule'
          action: 'Block'
          matchConditions: [
            {
              matchVariable: 'RequestUri'
              operator: 'Contains'
              negateCondition: false
              transforms: [
                'Lowercase'
                'UrlDecode'
              ]
              matchValue: [
                '/wp-admin'
                '/wp-login'
                '/xmlrpc'
                '.php'
                '/.env'
                '/.git'
                '/phpmyadmin'
                '/cgi-bin'
                '/admin'
                '/.aws'
                '/server-status'
                '/actuator'
              ]
            }
          ]
        }
        {
          name: 'RateLimitPerIpMinute'
          priority: 40
          enabledState: 'Enabled'
          ruleType: 'RateLimitRule'
          rateLimitDurationInMinutes: 1
          rateLimitThreshold: requestsPerIpPerMinute
          action: 'Block'
          groupBy: [
            {
              variableName: 'SocketAddr'
            }
          ]
          matchConditions: [
            {
              // Matches every request: "not in an impossible range".
              matchVariable: 'SocketAddr'
              operator: 'IPMatch'
              negateCondition: true
              matchValue: [
                '255.255.255.255/32'
              ]
            }
          ]
        }
        {
          name: 'RateLimitPerIp5Minutes'
          priority: 50
          enabledState: 'Enabled'
          ruleType: 'RateLimitRule'
          rateLimitDurationInMinutes: 5
          rateLimitThreshold: requestsPerIpPer5Minutes
          action: 'Block'
          groupBy: [
            {
              variableName: 'SocketAddr'
            }
          ]
          matchConditions: [
            {
              matchVariable: 'SocketAddr'
              operator: 'IPMatch'
              negateCondition: true
              matchValue: [
                '255.255.255.255/32'
              ]
            }
          ]
        }
      ])
    }
    managedRules: {
      managedRuleSets: [
        {
          ruleSetType: 'Microsoft_DefaultRuleSet'
          ruleSetVersion: '2.1'
          ruleSetAction: 'Block'
        }
        {
          ruleSetType: 'Microsoft_BotManagerRuleSet'
          ruleSetVersion: '1.1'
        }
      ]
    }
  }
}

resource profile 'Microsoft.Cdn/profiles@2024-02-01' = {
  name: '${name}-fd'
  location: 'global'
  sku: {
    name: 'Premium_AzureFrontDoor'
  }
}

resource endpoint 'Microsoft.Cdn/profiles/afdEndpoints@2024-02-01' = {
  parent: profile
  name: '${name}-ep'
  location: 'global'
  properties: {
    enabledState: 'Enabled'
  }
}

resource originGroup 'Microsoft.Cdn/profiles/originGroups@2024-02-01' = {
  parent: profile
  name: 'static-web-app'
  properties: {
    loadBalancingSettings: {
      sampleSize: 4
      successfulSamplesRequired: 3
      additionalLatencyInMilliseconds: 50
    }
    healthProbeSettings: {
      probePath: '/'
      probeRequestType: 'HEAD'
      probeProtocol: 'Https'
      probeIntervalInSeconds: 120
    }
  }
}

resource origin 'Microsoft.Cdn/profiles/originGroups/origins@2024-02-01' = {
  parent: originGroup
  name: 'swa'
  properties: {
    hostName: staticWebAppHostname
    originHostHeader: staticWebAppHostname
    httpPort: 80
    httpsPort: 443
    priority: 1
    weight: 1000
    enabledState: 'Enabled'
    enforceCertificateNameCheck: true
  }
}

resource route 'Microsoft.Cdn/profiles/afdEndpoints/routes@2024-02-01' = {
  parent: endpoint
  name: 'all'
  dependsOn: [
    origin
  ]
  properties: {
    originGroup: {
      id: originGroup.id
    }
    supportedProtocols: [
      'Http'
      'Https'
    ]
    patternsToMatch: [
      '/*'
    ]
    forwardingProtocol: 'HttpsOnly'
    httpsRedirect: 'Enabled'
    linkToDefaultDomain: 'Enabled'
    cacheConfiguration: {
      queryStringCachingBehavior: 'IgnoreQueryString'
      compressionSettings: {
        isCompressionEnabled: true
        contentTypesToCompress: [
          'text/html'
          'text/css'
          'text/javascript'
          'application/javascript'
          'image/svg+xml'
          'text/plain'
        ]
      }
    }
  }
}

resource securityPolicy 'Microsoft.Cdn/profiles/securityPolicies@2024-02-01' = {
  parent: profile
  name: 'waf'
  properties: {
    parameters: {
      type: 'WebApplicationFirewall'
      wafPolicy: {
        id: waf.id
      }
      associations: [
        {
          domains: [
            {
              id: endpoint.id
            }
          ]
          patternsToMatch: [
            '/*'
          ]
        }
      ]
    }
  }
}

@description('Put this in AFD_ID when building the site, so the Static Web App only accepts traffic from this Front Door.')
output frontDoorId string = profile.properties.frontDoorId

output frontDoorHostname string = endpoint.properties.hostName
